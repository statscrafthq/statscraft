//! Parsers of the time series commands: TSET and TSHOW (settings), CREATE (new series from
//! functions of others), ACF, PACF and CCF (correlograms) and SEASON (seasonal decomposition).
//!
//! Rules noted from the Command Syntax Reference:
//! - TSET takes its subcommands in any order, with or without slashes and equals signs:
//!   PRINT (BRIEF, DEFAULT, DETAILED), NEWVAR (CURRENT, NONE, ALL; NEWVARS is accepted too),
//!   MXAUTO, MXCROSS, MXNEWVARS, MXPREDICT (positive whole numbers), MISSING (EXCLUDE,
//!   INCLUDE), CIN (above 0, below 100), TOLER and CNVERGE (positive), ACFSE (IND, MA),
//!   PERIOD (a positive whole number), ID (a variable), CONSTANT or NOCONSTANT. DEFAULT as
//!   the value of a subcommand restores that one; DEFAULT on its own restores them all.
//! - TSHOW takes nothing.
//! - CREATE: `new names = FUNCTION(series [, numbers])`, equations separated by slashes.
//!   Names and numbers in the parentheses are separated by commas or blanks. CSUM, FFT, IFFT
//!   and T4253H take no number; DIFF and PMA one; LAG and LEAD one or two (a range); MA and
//!   RMED a span and an optional minimum span; SDIFF an order and an optional period.
//! - ACF, PACF and CCF: `[VARIABLES=] series` (CCF: two or more, or `a WITH b`), then DIFF[=n]
//!   and SDIFF[=n] (default 1, 0 allowed), PERIOD=n (above 1), LN or NOLOG, SEASONAL, MXAUTO=n
//!   (CCF: MXCROSS=n), APPLY [='name']; ACF also SERROR=IND|MA and PACF. VARIABLES is given
//!   once; any other subcommand may repeat and the last one counts.
//! - SEASON: `[VARIABLES=] series`, MODEL=MULTIPLICATIVE|ADDITIVE (of several the first
//!   counts; none: multiplicative), MA=EQUAL|CENTERED, PERIOD=n, APPLY [='name'].

use crate::ast::{Command, VarItem};
use crate::error::SyntaxError;
use crate::glm::list_until;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TsetPrint {
    Brief,
    Default,
    Detailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewVar {
    Current,
    None,
    All,
}

/// The settings of TSET, as the procedures read them.
#[derive(Debug, Clone, PartialEq)]
pub struct TimeSettings {
    pub print: TsetPrint,
    pub newvar: NewVar,
    pub mxauto: u32,
    pub mxcross: u32,
    pub mxnewvars: u32,
    pub mxpredict: u32,
    /// MISSING=INCLUDE: user-missing values count as valid.
    pub include: bool,
    pub cin: f64,
    pub toler: f64,
    pub cnverge: f64,
    /// ACFSE=MA
    pub acfse_ma: bool,
    pub period: Option<u32>,
    pub id: Option<String>,
    pub constant: bool,
}

impl Default for TimeSettings {
    fn default() -> Self {
        Self {
            print: TsetPrint::Default,
            newvar: NewVar::Current,
            mxauto: 16,
            mxcross: 7,
            mxnewvars: 60,
            mxpredict: 60,
            include: false,
            cin: 95.0,
            toler: 0.0001,
            cnverge: 0.001,
            acfse_ma: false,
            period: None,
            id: None,
            constant: true,
        }
    }
}

/// One subcommand of TSET; None restores the setting's default.
#[derive(Debug, Clone, PartialEq)]
pub enum TsetItem {
    /// DEFAULT on its own: every setting.
    Default,
    Print(Option<TsetPrint>),
    NewVar(Option<NewVar>),
    MxAuto(Option<u32>),
    MxCross(Option<u32>),
    MxNewVars(Option<u32>),
    MxPredict(Option<u32>),
    Include(Option<bool>),
    Cin(Option<f64>),
    Toler(Option<f64>),
    Cnverge(Option<f64>),
    AcfseMa(Option<bool>),
    Period(Option<u32>),
    Id(Option<String>),
    Constant(Option<bool>),
}

impl TimeSettings {
    pub fn apply(&mut self, item: &TsetItem) {
        let d = TimeSettings::default();
        match item.clone() {
            TsetItem::Default => *self = d,
            TsetItem::Print(v) => self.print = v.unwrap_or(d.print),
            TsetItem::NewVar(v) => self.newvar = v.unwrap_or(d.newvar),
            TsetItem::MxAuto(v) => self.mxauto = v.unwrap_or(d.mxauto),
            TsetItem::MxCross(v) => self.mxcross = v.unwrap_or(d.mxcross),
            TsetItem::MxNewVars(v) => self.mxnewvars = v.unwrap_or(d.mxnewvars),
            TsetItem::MxPredict(v) => self.mxpredict = v.unwrap_or(d.mxpredict),
            TsetItem::Include(v) => self.include = v.unwrap_or(d.include),
            TsetItem::Cin(v) => self.cin = v.unwrap_or(d.cin),
            TsetItem::Toler(v) => self.toler = v.unwrap_or(d.toler),
            TsetItem::Cnverge(v) => self.cnverge = v.unwrap_or(d.cnverge),
            TsetItem::AcfseMa(v) => self.acfse_ma = v.unwrap_or(d.acfse_ma),
            TsetItem::Period(v) => self.period = v,
            TsetItem::Id(v) => self.id = v,
            TsetItem::Constant(v) => self.constant = v.unwrap_or(d.constant),
        }
    }
}

/// The functions of CREATE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeriesFunction {
    Csum,
    Diff,
    Fft,
    Ifft,
    Lag,
    Lead,
    Ma,
    Pma,
    Rmed,
    Sdiff,
    T4253h,
}

const FUNCTIONS: [(&str, SeriesFunction); 11] = [
    ("CSUM", SeriesFunction::Csum),
    ("DIFF", SeriesFunction::Diff),
    ("FFT", SeriesFunction::Fft),
    ("IFFT", SeriesFunction::Ifft),
    ("LAG", SeriesFunction::Lag),
    ("LEAD", SeriesFunction::Lead),
    ("MA", SeriesFunction::Ma),
    ("PMA", SeriesFunction::Pma),
    ("RMED", SeriesFunction::Rmed),
    ("SDIFF", SeriesFunction::Sdiff),
    ("T4253H", SeriesFunction::T4253h),
];

impl SeriesFunction {
    pub fn name(self) -> &'static str {
        FUNCTIONS
            .iter()
            .find(|(_, f)| *f == self)
            .expect("listed")
            .0
    }

    /// The fewest and most numbers the function takes after its series.
    fn numbers(self) -> (usize, usize) {
        match self {
            Self::Csum | Self::Fft | Self::Ifft | Self::T4253h => (0, 0),
            Self::Diff | Self::Pma => (1, 1),
            Self::Lag | Self::Lead | Self::Ma | Self::Rmed | Self::Sdiff => (1, 2),
        }
    }
}

/// One equation of CREATE.
#[derive(Debug, Clone, PartialEq)]
pub struct CreateEquation {
    pub targets: Vec<String>,
    pub function: SeriesFunction,
    pub sources: Vec<VarItem>,
    /// The orders, spans or period, as written.
    pub numbers: Vec<u32>,
}

/// The functions of RMV.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RmvFunction {
    Lint,
    Mean,
    Median,
    Smean,
    Trend,
}

impl RmvFunction {
    pub fn name(self) -> &'static str {
        match self {
            Self::Lint => "LINT",
            Self::Mean => "MEAN",
            Self::Median => "MEDIAN",
            Self::Smean => "SMEAN",
            Self::Trend => "TREND",
        }
    }
}

/// How many valid cases MEAN and MEDIAN of RMV take on each side of a missing value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RmvSpan {
    Cases(u32),
    All,
}

/// One equation of RMV.
#[derive(Debug, Clone, PartialEq)]
pub struct RmvEquation {
    pub targets: Vec<String>,
    pub function: RmvFunction,
    pub sources: Vec<VarItem>,
    /// MEAN and MEDIAN: the span, 2 when none is written. SMEAN is MEAN with All.
    pub span: RmvSpan,
}

/// ACF, PACF or CCF.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorrelogramKind {
    Acf,
    Pacf,
    Ccf,
}

impl CorrelogramKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Acf => "ACF",
            Self::Pacf => "PACF",
            Self::Ccf => "CCF",
        }
    }
}

/// ACF, PACF or CCF as written: None for what the command leaves out, so that APPLY and TSET
/// can fill it in.
#[derive(Debug, Clone, PartialEq)]
pub struct Correlogram {
    pub kind: CorrelogramKind,
    pub variables: Vec<VarItem>,
    /// CCF: the series after WITH.
    pub with: Option<Vec<VarItem>>,
    /// APPLY: Some(None) for the previous command of the same kind, Some(Some(name)) for a
    /// named model.
    pub apply: Option<Option<String>>,
    pub diff: Option<u32>,
    pub sdiff: Option<u32>,
    pub period: Option<u32>,
    pub ln: Option<bool>,
    pub seasonal: Option<bool>,
    /// MXAUTO, or MXCROSS for CCF.
    pub max_lag: Option<u32>,
    /// SERROR=MA (ACF).
    pub ma_se: Option<bool>,
    /// The PACF subcommand of ACF.
    pub pacf: Option<bool>,
}

/// SEASON as written.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Season {
    pub variables: Vec<VarItem>,
    pub apply: Option<Option<String>>,
    pub additive: Option<bool>,
    /// MA=CENTERED (true) or EQUAL (false).
    pub centered: Option<bool>,
    pub period: Option<u32>,
}

fn error(p: &Parser, start: usize, message: impl Into<String>) -> SyntaxError {
    SyntaxError::new(p.span_from(start), message.into())
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

/// `[=] DEFAULT` gives None; otherwise what `value` reads.
fn or_default<T>(
    p: &mut Parser,
    value: impl FnOnce(&mut Parser) -> PResult<T>,
) -> PResult<Option<T>> {
    p.eat(Punct::Eq);
    if p.eat_keyword("DEFAULT") {
        return Ok(None);
    }
    value(p).map(Some)
}

fn pick<T: Copy>(p: &mut Parser, choices: &[(&'static str, T)]) -> PResult<T> {
    match choices.iter().find(|(k, _)| p.at_keyword(k)) {
        Some((_, v)) => {
            p.pos += 1;
            Ok(*v)
        }
        None => Err(p.unexpected(&choices.iter().map(|(k, _)| *k).collect::<Vec<_>>())),
    }
}

fn positive(p: &mut Parser, what: &str) -> PResult<f64> {
    let start = p.pos;
    match p.number() {
        Some(v) if v > 0.0 => Ok(v),
        Some(_) => Err(error(p, start, format!("{what} must be above 0"))),
        None => Err(p.unexpected(&["a number"])),
    }
}

pub(crate) fn tset(p: &mut Parser) -> PResult<Command> {
    let mut items = Vec::new();
    loop {
        p.eat(Punct::Slash);
        if p.peek().is_none() {
            break;
        }
        let item = if p.eat_keyword("PRINT") {
            TsetItem::Print(or_default(p, |p| {
                pick(
                    p,
                    &[
                        ("BRIEF", TsetPrint::Brief),
                        ("DETAILED", TsetPrint::Detailed),
                    ],
                )
            })?)
        } else if p.eat_word("NEWVARS") || p.eat_keyword("NEWVAR") {
            TsetItem::NewVar(or_default(p, |p| {
                pick(
                    p,
                    &[
                        ("CURRENT", NewVar::Current),
                        ("NONE", NewVar::None),
                        ("ALL", NewVar::All),
                    ],
                )
            })?)
        } else if p.eat_keyword("MXAUTO") {
            TsetItem::MxAuto(or_default(p, |p| whole(p, 1, "MXAUTO"))?)
        } else if p.eat_keyword("MXCROSS") {
            TsetItem::MxCross(or_default(p, |p| whole(p, 1, "MXCROSS"))?)
        } else if p.eat_keyword("MXNEWVARS") {
            TsetItem::MxNewVars(or_default(p, |p| whole(p, 1, "MXNEWVARS"))?)
        } else if p.eat_keyword("MXPREDICT") {
            TsetItem::MxPredict(or_default(p, |p| whole(p, 1, "MXPREDICT"))?)
        } else if p.eat_keyword("MISSING") {
            TsetItem::Include(or_default(p, |p| {
                pick(p, &[("EXCLUDE", false), ("INCLUDE", true)])
            })?)
        } else if p.eat_keyword("CIN") {
            TsetItem::Cin(or_default(p, |p| {
                let at = p.pos;
                match p.number() {
                    Some(v) if v > 0.0 && v < 100.0 => Ok(v),
                    Some(_) => Err(error(p, at, "CIN is above 0 and below 100")),
                    None => Err(p.unexpected(&["a percentage"])),
                }
            })?)
        } else if p.eat_keyword("TOLER") {
            TsetItem::Toler(or_default(p, |p| positive(p, "TOLER"))?)
        } else if p.eat_keyword("CNVERGE") {
            TsetItem::Cnverge(or_default(p, |p| positive(p, "CNVERGE"))?)
        } else if p.eat_keyword("ACFSE") {
            TsetItem::AcfseMa(or_default(p, |p| pick(p, &[("IND", false), ("MA", true)]))?)
        } else if p.eat_keyword("PERIOD") {
            TsetItem::Period(or_default(p, |p| whole(p, 1, "PERIOD"))?)
        } else if p.eat_word("ID") {
            TsetItem::Id(or_default(p, |p| p.name("a variable name"))?)
        } else if p.eat_keyword("CONSTANT") {
            TsetItem::Constant(Some(true))
        } else if p.eat_keyword("NOCONSTANT") {
            TsetItem::Constant(Some(false))
        } else if p.eat_keyword("DEFAULT") {
            TsetItem::Default
        } else {
            return Err(p.unexpected(&[
                "PRINT",
                "NEWVAR",
                "MXAUTO",
                "MXCROSS",
                "MXNEWVARS",
                "MXPREDICT",
                "MISSING",
                "CIN",
                "TOLER",
                "CNVERGE",
                "ACFSE",
                "PERIOD",
                "ID",
                "CONSTANT",
                "NOCONSTANT",
                "DEFAULT",
            ]));
        };
        items.push(item);
    }
    if items.is_empty() {
        return Err(p.unexpected(&["a TSET subcommand"]));
    }
    Ok(Command::Tset(items))
}

pub(crate) fn tshow(p: &mut Parser) -> PResult<Command> {
    p.finish(&[])?;
    Ok(Command::Tshow)
}

pub(crate) fn verify(p: &mut Parser) -> PResult<Command> {
    let named = p.at_keyword("VARIABLES") && p.peek_at(1) == Some(&TokenKind::Punct(Punct::Eq));
    p.pos += 2 * usize::from(named);
    let series = match p.peek() {
        None if !named => None,
        _ => Some(p.name("a series name")?),
    };
    p.finish(&[])?;
    Ok(Command::Verify(series))
}

pub(crate) fn create(p: &mut Parser) -> PResult<Command> {
    let mut equations = Vec::new();
    loop {
        let mut targets = Vec::new();
        while matches!(p.peek(), Some(TokenKind::Word(_))) {
            let start = p.pos;
            let first = p.name("a new series name")?;
            if p.eat_word("TO") {
                let last = p.name("a new series name")?;
                targets.extend(
                    crate::data_list::numbered_names(&first, &last)
                        .map_err(|e| error(p, start, e))?,
                );
            } else {
                targets.push(first);
            }
            p.eat(Punct::Comma);
        }
        if targets.is_empty() {
            return Err(p.unexpected(&["a new series name"]));
        }
        p.expect(Punct::Eq)?;
        let start = p.pos;
        let function = pick(p, &FUNCTIONS)?;
        p.expect(Punct::LParen)?;
        let sources = list_until(p, &[])?;
        let mut numbers = Vec::new();
        while !p.eat(Punct::RParen) {
            p.eat(Punct::Comma);
            if p.eat(Punct::RParen) {
                break;
            }
            numbers.push(whole(p, 0, function.name())?);
        }
        let (low, high) = function.numbers();
        if numbers.len() < low || numbers.len() > high {
            let wanted = match (low, high) {
                (0, 0) => "no number after its series".to_string(),
                (1, 1) => "one number after its series".to_string(),
                _ => "one or two numbers after its series".to_string(),
            };
            return Err(error(
                p,
                start,
                format!("{} takes {wanted}", function.name()),
            ));
        }
        let first_low = match function {
            SeriesFunction::Lag | SeriesFunction::Lead => 0,
            _ => 1,
        };
        if numbers.first().is_some_and(|&n| n < first_low) {
            return Err(error(
                p,
                start,
                format!("the first number of {} must be at least 1", function.name()),
            ));
        }
        match (function, numbers.as_slice()) {
            (SeriesFunction::Lag | SeriesFunction::Lead, [a, b]) if b < a => {
                return Err(error(
                    p,
                    start,
                    format!(
                        "the range of {} runs from the lower order to the higher",
                        function.name()
                    ),
                ));
            }
            (SeriesFunction::Ma | SeriesFunction::Rmed, [span, minimum])
                if *minimum < 1 || minimum > span =>
            {
                return Err(error(
                    p,
                    start,
                    format!(
                        "the minimum span of {} is from 1 to the span",
                        function.name()
                    ),
                ));
            }
            (SeriesFunction::Sdiff, [_, 0]) => {
                return Err(error(p, start, "the period of SDIFF must be at least 1"));
            }
            _ => {}
        }
        equations.push(CreateEquation {
            targets,
            function,
            sources,
            numbers,
        });
        if !p.eat(Punct::Slash) {
            break;
        }
    }
    p.finish(&["'/'"])?;
    Ok(Command::Create(equations))
}

const RMV_FUNCTIONS: [(&str, RmvFunction); 5] = [
    ("LINT", RmvFunction::Lint),
    ("MEAN", RmvFunction::Mean),
    ("MEDIAN", RmvFunction::Median),
    ("SMEAN", RmvFunction::Smean),
    ("TREND", RmvFunction::Trend),
];

pub(crate) fn rmv(p: &mut Parser) -> PResult<Command> {
    let mut equations = Vec::new();
    loop {
        let mut targets = Vec::new();
        while matches!(p.peek(), Some(TokenKind::Word(_))) {
            let start = p.pos;
            let first = p.name("a new variable name")?;
            if p.eat_word("TO") {
                let last = p.name("a new variable name")?;
                targets.extend(
                    crate::data_list::numbered_names(&first, &last)
                        .map_err(|e| error(p, start, e))?,
                );
            } else {
                targets.push(first);
            }
            p.eat(Punct::Comma);
        }
        if targets.is_empty() {
            return Err(p.unexpected(&["a new variable name"]));
        }
        p.expect(Punct::Eq)?;
        let function = pick(p, &RMV_FUNCTIONS)?;
        p.expect(Punct::LParen)?;
        // ALL after the list is the span, not a list of all variables
        let sources = list_until(p, &["ALL"])?;
        let mut span = match function {
            RmvFunction::Smean => RmvSpan::All,
            _ => RmvSpan::Cases(2),
        };
        let has_span = matches!(function, RmvFunction::Mean | RmvFunction::Median);
        let start = p.pos;
        if p.at_keyword("ALL") || matches!(p.peek(), Some(TokenKind::Number(_))) {
            if !has_span {
                return Err(error(
                    p,
                    start,
                    format!("{} takes only a list of variables", function.name()),
                ));
            }
            span = if p.eat_word("ALL") {
                RmvSpan::All
            } else {
                RmvSpan::Cases(whole(p, 1, "the span")?)
            };
        }
        p.expect(Punct::RParen)?;
        equations.push(RmvEquation {
            targets,
            function,
            sources,
            span,
        });
        if !p.eat(Punct::Slash) {
            break;
        }
    }
    p.finish(&["'/'"])?;
    Ok(Command::Rmv(equations))
}

/// `[=] n`, or `default` when no number follows.
fn count(p: &mut Parser, default: Option<u32>, low: u32, what: &str) -> PResult<u32> {
    p.eat(Punct::Eq);
    if (p.peek().is_none() || p.at(Punct::Slash))
        && let Some(d) = default
    {
        return Ok(d);
    }
    whole(p, low, what)
}

fn apply(p: &mut Parser) -> PResult<Option<String>> {
    p.eat(Punct::Eq);
    Ok(match p.peek() {
        Some(TokenKind::Str(_)) => Some(p.string("a model name")?),
        _ => None,
    })
}

/// The leading variable list, after an optional `VARIABLES =`; CCF also reads `WITH`.
fn series_list(p: &mut Parser, with: bool) -> PResult<(Vec<VarItem>, Option<Vec<VarItem>>)> {
    let stop: &[&str] = if with { &["WITH"] } else { &[] };
    let left = list_until(p, stop)?;
    let right = match with && p.eat_word("WITH") {
        true => Some(list_until(p, &[])?),
        false => None,
    };
    Ok((left, right))
}

/// Whether the command continues with its variable list here, without a slash.
fn at_variables(p: &Parser) -> bool {
    if p.at_keyword("VARIABLES") && p.peek_at(1) == Some(&TokenKind::Punct(Punct::Eq)) {
        return true;
    }
    matches!(p.peek(), Some(TokenKind::Word(_))) && !p.at_keyword("APPLY")
}

fn correlogram(p: &mut Parser, kind: CorrelogramKind) -> PResult<Command> {
    let mut command = Correlogram {
        kind,
        variables: Vec::new(),
        with: None,
        apply: None,
        diff: None,
        sdiff: None,
        period: None,
        ln: None,
        seasonal: None,
        max_lag: None,
        ma_se: None,
        pacf: None,
    };
    let name = kind.name();
    let mut first = true;
    loop {
        if !first && !p.eat(Punct::Slash) {
            break;
        }
        if first && p.eat(Punct::Slash) {
            first = false;
        }
        let start = p.pos;
        if (first && at_variables(p)) || p.at_keyword("VARIABLES") {
            if !command.variables.is_empty() {
                return Err(error(p, start, "VARIABLES is given only once"));
            }
            if p.at_keyword("VARIABLES") && p.peek_at(1) == Some(&TokenKind::Punct(Punct::Eq)) {
                p.pos += 2;
            }
            let (left, right) = series_list(p, kind == CorrelogramKind::Ccf)?;
            command.variables = left;
            command.with = right;
        } else if p.eat_keyword("APPLY") {
            command.apply = Some(apply(p)?);
        } else if p.eat_keyword("DIFFERENCE") {
            command.diff = Some(count(p, Some(1), 0, "DIFF")?);
        } else if p.eat_keyword("SDIFF") {
            command.sdiff = Some(count(p, Some(1), 0, "SDIFF")?);
        } else if p.eat_keyword("PERIOD") {
            command.period = Some(count(p, None, 2, "PERIOD")?);
        } else if p.eat_word("LN") {
            command.ln = Some(true);
        } else if p.eat_keyword("NOLOG") {
            command.ln = Some(false);
        } else if p.eat_keyword("SEASONAL") {
            command.seasonal = Some(true);
        } else if kind != CorrelogramKind::Ccf && p.eat_keyword("MXAUTO") {
            command.max_lag = Some(count(p, None, 1, "MXAUTO")?);
        } else if kind == CorrelogramKind::Ccf && p.eat_keyword("MXCROSS") {
            command.max_lag = Some(count(p, None, 1, "MXCROSS")?);
        } else if kind == CorrelogramKind::Acf && p.eat_keyword("SERROR") {
            p.eat(Punct::Eq);
            command.ma_se = Some(pick(p, &[("IND", false), ("MA", true)])?);
        } else if kind == CorrelogramKind::Acf && p.eat_keyword("PACF") {
            command.pacf = Some(true);
        } else {
            return Err(p.subcommand_error(name, &[]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
        first = false;
    }
    p.finish(&["'/'"])?;
    if command.variables.is_empty() && command.apply.is_none() {
        return Err(SyntaxError::new(
            p.span(),
            format!("{name} needs VARIABLES, or APPLY to use the previous {name}'s"),
        ));
    }
    Ok(Command::Correlogram(Box::new(command)))
}

pub(crate) fn acf(p: &mut Parser) -> PResult<Command> {
    correlogram(p, CorrelogramKind::Acf)
}

pub(crate) fn pacf(p: &mut Parser) -> PResult<Command> {
    correlogram(p, CorrelogramKind::Pacf)
}

pub(crate) fn ccf(p: &mut Parser) -> PResult<Command> {
    correlogram(p, CorrelogramKind::Ccf)
}

pub(crate) fn season(p: &mut Parser) -> PResult<Command> {
    let mut command = Season::default();
    let mut first = true;
    loop {
        if !first && !p.eat(Punct::Slash) {
            break;
        }
        if first && p.eat(Punct::Slash) {
            first = false;
        }
        let start = p.pos;
        if (first && at_variables(p)) || p.at_keyword("VARIABLES") {
            if !command.variables.is_empty() {
                return Err(error(p, start, "VARIABLES is given only once"));
            }
            if p.at_keyword("VARIABLES") && p.peek_at(1) == Some(&TokenKind::Punct(Punct::Eq)) {
                p.pos += 2;
            }
            command.variables = list_until(p, &[])?;
        } else if p.eat_keyword("APPLY") {
            command.apply = Some(apply(p)?);
        } else if p.eat_keyword("MODEL") {
            p.eat(Punct::Eq);
            let mut additive = None;
            while !at_end(p) {
                let a = pick(p, &[("MULTIPLICATIVE", false), ("ADDITIVE", true)])?;
                // of several keywords the first counts
                additive.get_or_insert(a);
                p.eat(Punct::Comma);
            }
            command.additive = Some(additive.unwrap_or(false));
        } else if p.eat_word("MA") {
            p.eat(Punct::Eq);
            command.centered = Some(pick(p, &[("EQUAL", false), ("CENTERED", true)])?);
        } else if p.eat_keyword("PERIOD") {
            command.period = Some(count(p, None, 1, "PERIOD")?);
        } else {
            return Err(p.subcommand_error("SEASON", &[]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
        first = false;
    }
    p.finish(&["'/'"])?;
    if command.variables.is_empty() && command.apply.is_none() {
        return Err(SyntaxError::new(
            p.span(),
            "SEASON needs VARIABLES, or APPLY to use the previous SEASON's",
        ));
    }
    Ok(Command::Season(Box::new(command)))
}
