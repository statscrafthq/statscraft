//! Parsers of KM (product-limit survival estimates) and SURVIVAL (actuarial life tables).
//!
//! KM rules, in our words:
//! - `time [BY factor]` comes first; one time variable and at most one factor. K-M is
//!   another name for the command.
//! - STATUS is required: one variable, `[EVENT](values)` and optionally `LOST(values)`. Values
//!   are single values or numeric `a THRU b` ranges, with or without commas; a string status
//!   takes quoted values. A repeated STATUS replaces the earlier one.
//! - STRATA and ID name one variable each. PLOT takes SURVIVAL, LOGSURV, HAZARD and OMS; alone
//!   it means SURVIVAL. PRINT takes TABLE, MEAN and NONE; once given, only the named parts show.
//! - PERCENTILES takes values above 0 and below 100, optionally in parentheses; alone it means
//!   25, 50 and 75. TEST takes LOGRANK, BRESLOW and TARONE; alone it means LOGRANK.
//! - COMPARE takes OVERALL or PAIRWISE and POOLED or STRATA; it may be repeated, each one adding
//!   a comparison. TREND takes an optional list of weights in parentheses.
//! - SAVE lists SURVIVAL, HAZARD, SE and CUMEVENT, each at most once, each with an optional
//!   new name in parentheses.
//!
//! SURVIVAL rules, in our words:
//! - `TABLE=` survival variables, then up to two `BY` lists of control variables, each list a
//!   run of names closed by `(min, max)` whole numbers that apply to the names before them.
//! - INTERVAL is required and starts with THRU: `THRU end BY width`, repeated for wider steps
//!   further out. STATUS is required: `var(min, max)` or `var(value)`, optionally `FOR`
//!   survival variables; several STATUS subcommands give each survival variable its own.
//! - PLOTS `[(kinds)] [= survival BY controls BY controls]` with kinds ALL, LOGSURV, SURVIVAL,
//!   HAZARD, DENSITY and OMS. PRINT takes TABLE or NOTABLE. COMPARE takes an optional list of
//!   the same shape as PLOTS. CALCULATE takes EXACT, CONDITIONAL or APPROXIMATE (of several the
//!   strongest in that reverse order counts), PAIRWISE and COMPARE. MISSING takes GROUPWISE or
//!   LISTWISE and INCLUDE. WRITE takes NONE, TABLES or BOTH; alone it means TABLES. Only NONE
//!   is supported: the others write through PROCEDURE OUTPUT, which Statscraft lacks.

use crate::ast::{Command, MissingValue, VarItem};
use crate::error::SyntaxError;
use crate::glm::list_until;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KmPlot {
    Survival,
    LogSurvival,
    Hazard,
    OneMinusSurvival,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KmTest {
    Logrank,
    Breslow,
    Tarone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KmSave {
    Survival,
    Hazard,
    Se,
    CumEvent,
}

/// One requested comparison of the factor levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KmCompare {
    /// Each pair of levels rather than all levels at once.
    pub pairwise: bool,
    /// Within each stratum rather than pooled over the strata.
    pub per_stratum: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Km {
    pub time: String,
    pub factor: Option<String>,
    pub status: String,
    /// STATUS was given more than once; the last one counts.
    pub status_repeated: bool,
    pub event: Vec<MissingValue>,
    pub lost: Vec<MissingValue>,
    pub strata: Option<String>,
    pub id: Option<String>,
    pub plots: Vec<KmPlot>,
    pub table: bool,
    pub mean: bool,
    /// Percentages, such as 25 for the time by which survival falls to 25%.
    pub percentiles: Vec<f64>,
    /// The tests asked for; empty means the log rank test.
    pub tests: Vec<KmTest>,
    /// Empty means one overall comparison pooled over the strata.
    pub compare: Vec<KmCompare>,
    /// TREND: the weights of the levels, empty for the default weights.
    pub trend: Option<Vec<f64>>,
    pub save: Vec<(KmSave, Option<String>)>,
}

/// A control variable of SURVIVAL with its range of whole values.
#[derive(Debug, Clone, PartialEq)]
pub struct Control {
    pub variable: VarItem,
    pub min: i64,
    pub max: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SurvivalStatus {
    pub variable: String,
    /// The event codes run from the first to the second, both included.
    pub min: f64,
    pub max: f64,
    /// The survival variables it applies to; None for all not named elsewhere.
    pub applies_to: Option<Vec<VarItem>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifePlot {
    Survival,
    LogSurvival,
    Hazard,
    Density,
    OneMinusSurvival,
}

/// A selection of the tables: survival variables, then the first and second order control
/// lists; a missing or `ALL` list selects every variable of that place.
pub type TableSelection = Vec<Vec<VarItem>>;

#[derive(Debug, Clone, PartialEq)]
pub struct Survival {
    pub variables: Vec<VarItem>,
    /// The first and the second order control lists; empty when there is no BY.
    pub controls: Vec<Vec<Control>>,
    /// `(end, width)` pairs in the order given.
    pub intervals: Vec<(f64, f64)>,
    pub status: Vec<SurvivalStatus>,
    /// PLOTS: the functions and which tables; None without PLOTS.
    pub plots: Option<(Vec<LifePlot>, TableSelection)>,
    pub table: bool,
    pub compare: Option<TableSelection>,
    /// CALCULATE=APPROXIMATE: compare by the intervals the times fall into.
    pub approximate: bool,
    pub pairwise: bool,
    /// CALCULATE=COMPARE: the comparisons only.
    pub compare_only: bool,
    pub listwise: bool,
    pub include: bool,
}

/// `(values)` of KM's STATUS: values and numeric ranges.
fn value_list(p: &mut Parser) -> PResult<Vec<MissingValue>> {
    p.expect(Punct::LParen)?;
    let mut values = Vec::new();
    while let Some(value) = p.value_or_range()? {
        values.push(value);
        p.eat(Punct::Comma);
    }
    if values.is_empty() {
        return Err(p.unexpected(&["a value"]));
    }
    p.expect(Punct::RParen)?;
    Ok(values)
}

/// A number written in parentheses-free lists: PERCENTILES and TREND.
fn numbers(p: &mut Parser) -> Vec<f64> {
    let mut out = Vec::new();
    while let Some(n) = p.number() {
        out.push(n);
        p.eat(Punct::Comma);
    }
    out
}

pub(crate) fn km(p: &mut Parser) -> PResult<Command> {
    let time = p.name("the survival time variable")?;
    let factor = match p.eat_word("BY") {
        true => Some(p.name("the factor variable")?),
        false => None,
    };
    let mut command = Km {
        time,
        factor,
        status: String::new(),
        status_repeated: false,
        event: Vec::new(),
        lost: Vec::new(),
        strata: None,
        id: None,
        plots: Vec::new(),
        table: true,
        mean: true,
        percentiles: Vec::new(),
        tests: Vec::new(),
        compare: Vec::new(),
        trend: None,
        save: Vec::new(),
    };
    while p.eat(Punct::Slash) {
        if p.eat_keyword("STATUS") {
            p.eat(Punct::Eq);
            command.status_repeated = !command.status.is_empty();
            command.status = p.name("the status variable")?;
            p.eat_keyword("EVENT");
            command.event = value_list(p)?;
            command.lost = match p.eat_keyword("LOST") {
                true => value_list(p)?,
                false => Vec::new(),
            };
        } else if p.eat_keyword("STRATA") {
            p.eat(Punct::Eq);
            command.strata = Some(p.name("the strata variable")?);
        } else if p.eat_word("ID") {
            p.eat(Punct::Eq);
            command.id = Some(p.name("the ID variable")?);
        } else if p.eat_keyword("PLOT") {
            p.eat(Punct::Eq);
            const KINDS: [(&str, KmPlot); 4] = [
                ("SURVIVAL", KmPlot::Survival),
                ("LOGSURV", KmPlot::LogSurvival),
                ("HAZARD", KmPlot::Hazard),
                ("OMS", KmPlot::OneMinusSurvival),
            ];
            let before = command.plots.len();
            while !at_end(p) {
                let Some(&(_, kind)) = KINDS.iter().find(|(k, _)| p.at_keyword(k)) else {
                    return Err(p.unexpected(&KINDS.map(|k| k.0)));
                };
                p.pos += 1;
                if !command.plots.contains(&kind) {
                    command.plots.push(kind);
                }
                p.eat(Punct::Comma);
            }
            if command.plots.len() == before && !command.plots.contains(&KmPlot::Survival) {
                command.plots.push(KmPlot::Survival);
            }
        } else if p.eat_keyword("PRINT") {
            p.eat(Punct::Eq);
            (command.table, command.mean) = (false, false);
            while !at_end(p) {
                if p.eat_keyword("TABLE") {
                    command.table = true;
                } else if p.eat_keyword("MEAN") {
                    command.mean = true;
                } else if p.eat_keyword("NONE") {
                    (command.table, command.mean) = (false, false);
                } else {
                    return Err(p.unexpected(&["TABLE", "MEAN", "NONE"]));
                }
                p.eat(Punct::Comma);
            }
        } else if p.eat_keyword("PERCENTILES") {
            p.eat(Punct::Eq);
            let parens = p.eat(Punct::LParen);
            let start = p.pos;
            let values = numbers(p);
            if let Some(bad) = values.iter().find(|v| !(**v > 0.0 && **v < 100.0)) {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    format!("PERCENTILES takes values above 0 and below 100, got {bad}"),
                ));
            }
            if parens {
                p.expect(Punct::RParen)?;
            }
            command.percentiles = match values.is_empty() {
                true => vec![25.0, 50.0, 75.0],
                false => values,
            };
        } else if p.eat_keyword("TEST") {
            p.eat(Punct::Eq);
            command.tests.clear();
            while !at_end(p) {
                let test = if p.eat_keyword("LOGRANK") {
                    KmTest::Logrank
                } else if p.eat_keyword("BRESLOW") {
                    KmTest::Breslow
                } else if p.eat_keyword("TARONE") {
                    KmTest::Tarone
                } else {
                    return Err(p.unexpected(&["LOGRANK", "BRESLOW", "TARONE"]));
                };
                if !command.tests.contains(&test) {
                    command.tests.push(test);
                }
                p.eat(Punct::Comma);
            }
        } else if p.eat_keyword("COMPARE") {
            p.eat(Punct::Eq);
            let mut compare = KmCompare {
                pairwise: false,
                per_stratum: false,
            };
            while !at_end(p) {
                if p.eat_keyword("OVERALL") {
                    compare.pairwise = false;
                } else if p.eat_keyword("PAIRWISE") {
                    compare.pairwise = true;
                } else if p.eat_keyword("POOLED") {
                    compare.per_stratum = false;
                } else if p.eat_keyword("STRATA") {
                    compare.per_stratum = true;
                } else {
                    return Err(p.unexpected(&["OVERALL", "PAIRWISE", "POOLED", "STRATA"]));
                }
                p.eat(Punct::Comma);
            }
            if !command.compare.contains(&compare) {
                command.compare.push(compare);
            }
        } else if p.eat_keyword("TREND") {
            p.eat(Punct::Eq);
            let mut weights = Vec::new();
            if p.eat(Punct::LParen) {
                weights = numbers(p);
                if weights.is_empty() {
                    return Err(p.unexpected(&["a trend weight"]));
                }
                p.expect(Punct::RParen)?;
            }
            command.trend = Some(weights);
        } else if p.eat_keyword("SAVE") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                let start = p.pos;
                let kind = if p.eat_keyword("SURVIVAL") {
                    KmSave::Survival
                } else if p.eat_keyword("HAZARD") {
                    KmSave::Hazard
                } else if p.eat_word("SE") {
                    KmSave::Se
                } else if p.eat_keyword("CUMEVENT") {
                    KmSave::CumEvent
                } else {
                    return Err(p.unexpected(&["SURVIVAL", "HAZARD", "SE", "CUMEVENT"]));
                };
                if command.save.iter().any(|(k, _)| *k == kind) {
                    return Err(SyntaxError::new(
                        p.span_from(start),
                        "SAVE names each of SURVIVAL, HAZARD, SE and CUMEVENT at most once",
                    ));
                }
                let name = match p.eat(Punct::LParen) {
                    true => {
                        let name = p.name("a new variable name")?;
                        if name.starts_with(['#', '$']) {
                            return Err(SyntaxError::new(
                                p.span_from(start),
                                format!("SAVE cannot make the scratch or system variable {name}"),
                            ));
                        }
                        p.expect(Punct::RParen)?;
                        Some(name)
                    }
                    false => None,
                };
                command.save.push((kind, name));
                p.eat(Punct::Comma);
            }
        } else {
            return Err(p.subcommand_error("KM", &[]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    if command.status.is_empty() {
        return Err(SyntaxError::new(
            p.span(),
            "KM needs STATUS: the status variable and its event values",
        ));
    }
    Ok(Command::Km(Box::new(command)))
}

/// A whole number of a control range.
fn whole(p: &mut Parser) -> PResult<i64> {
    let start = p.pos;
    match p.number() {
        Some(n) if n.fract() == 0.0 && n.abs() < 1e15 => Ok(n as i64),
        Some(n) => Err(SyntaxError::new(
            p.span_from(start),
            format!("a control range takes whole numbers, got {n}"),
        )),
        None => Err(p.unexpected(&["a whole number"])),
    }
}

/// Control variables up to the next BY or the end of the subcommand: names, each run closed
/// by `(min, max)`.
fn controls(p: &mut Parser) -> PResult<Vec<Control>> {
    let mut out = Vec::new();
    loop {
        let names = list_until(p, &["BY"])?;
        if !p.eat(Punct::LParen) {
            return Err(p.unexpected(&["'(' and the range of the control values"]));
        }
        let min = whole(p)?;
        p.eat(Punct::Comma);
        let max = whole(p)?;
        p.expect(Punct::RParen)?;
        if min > max {
            return Err(SyntaxError::new(
                p.span(),
                format!("the control range ({min}, {max}) runs backward"),
            ));
        }
        out.extend(
            names
                .into_iter()
                .map(|variable| Control { variable, min, max }),
        );
        p.eat(Punct::Comma);
        if !matches!(p.peek(), Some(TokenKind::Word(w)) if !w.eq_ignore_ascii_case("BY")) {
            return Ok(out);
        }
    }
}

/// `survival BY controls BY controls` without ranges, as PLOTS and COMPARE select tables.
fn selection(p: &mut Parser) -> PResult<TableSelection> {
    let mut lists = Vec::new();
    if at_end(p) {
        return Ok(lists);
    }
    lists.push(list_until(p, &["BY"])?);
    while p.eat_word("BY") {
        lists.push(list_until(p, &["BY"])?);
    }
    Ok(lists)
}

pub(crate) fn survival(p: &mut Parser) -> PResult<Command> {
    // TABLES= may be left out; without '=' a word is the first survival variable
    if p.peek_at(1) == Some(&TokenKind::Punct(Punct::Eq)) {
        p.eat_keyword("TABLES");
        p.eat(Punct::Eq);
    }
    let variables = list_until(p, &["BY"])?;
    let mut command = Survival {
        variables,
        controls: Vec::new(),
        intervals: Vec::new(),
        status: Vec::new(),
        plots: None,
        table: true,
        compare: None,
        approximate: false,
        pairwise: false,
        compare_only: false,
        listwise: false,
        include: false,
    };
    while p.eat_word("BY") {
        if command.controls.len() == 2 {
            return Err(p.unexpected(&["'/'"]));
        }
        command.controls.push(controls(p)?);
    }
    let mut calculate = false;
    while p.eat(Punct::Slash) {
        if p.eat_keyword("INTERVALS") {
            p.eat(Punct::Eq);
            command.intervals.clear();
            let mut last = 0.0;
            while !at_end(p) {
                if !p.eat_keyword("THRU") {
                    return Err(p.unexpected(&["THRU"]));
                }
                let start = p.pos;
                let Some(end) = p.number() else {
                    return Err(p.unexpected(&["the end of the period"]));
                };
                if !p.eat_word("BY") {
                    return Err(p.unexpected(&["BY"]));
                }
                let Some(width) = p.number() else {
                    return Err(p.unexpected(&["the width of the intervals"]));
                };
                if width <= 0.0 || end <= last {
                    return Err(SyntaxError::new(
                        p.span_from(start),
                        "INTERVAL periods must end in ascending order above 0, with widths above 0",
                    ));
                }
                last = end;
                command.intervals.push((end, width));
                p.eat(Punct::Comma);
            }
            if command.intervals.is_empty() {
                return Err(p.unexpected(&["THRU"]));
            }
        } else if p.eat_keyword("STATUS") {
            p.eat(Punct::Eq);
            let variable = p.name("the status variable")?;
            p.expect(Punct::LParen)?;
            let Some(min) = p.number() else {
                return Err(p.unexpected(&["an event code"]));
            };
            p.eat(Punct::Comma);
            let max = p.number().unwrap_or(min);
            p.expect(Punct::RParen)?;
            let applies_to = match p.eat_word("FOR") {
                true => Some(list_until(p, &[])?),
                false => None,
            };
            command.status.push(SurvivalStatus {
                variable,
                min: min.min(max),
                max: min.max(max),
                applies_to,
            });
        } else if p.eat_keyword("PLOTS") {
            let mut kinds = Vec::new();
            if p.eat(Punct::LParen) {
                const KINDS: [(&str, &[LifePlot]); 6] = [
                    ("ALL", &ALL_PLOTS),
                    ("LOGSURV", &[LifePlot::LogSurvival]),
                    ("SURVIVAL", &[LifePlot::Survival]),
                    ("HAZARD", &[LifePlot::Hazard]),
                    ("DENSITY", &[LifePlot::Density]),
                    ("OMS", &[LifePlot::OneMinusSurvival]),
                ];
                while !p.eat(Punct::RParen) {
                    let Some((_, add)) = KINDS.iter().find(|(k, _)| p.at_keyword(k)) else {
                        return Err(p.unexpected(&KINDS.map(|k| k.0)));
                    };
                    p.pos += 1;
                    for kind in add.iter() {
                        if !kinds.contains(kind) {
                            kinds.push(*kind);
                        }
                    }
                    p.eat(Punct::Comma);
                }
            }
            if kinds.is_empty() {
                kinds = ALL_PLOTS.to_vec();
            }
            p.eat(Punct::Eq);
            command.plots = Some((kinds, selection(p)?));
        } else if p.eat_keyword("PRINT") {
            p.eat(Punct::Eq);
            if p.eat_keyword("TABLE") {
                command.table = true;
            } else if p.eat_keyword("NOTABLE") {
                command.table = false;
            } else {
                return Err(p.unexpected(&["TABLE", "NOTABLE"]));
            }
        } else if p.eat_keyword("COMPARE") {
            p.eat(Punct::Eq);
            command.compare = Some(selection(p)?);
        } else if p.eat_keyword("CALCULATE") {
            p.eat(Punct::Eq);
            calculate = true;
            let mut approximate = false;
            while !at_end(p) {
                // there is always room for exact comparisons, so CONDITIONAL is EXACT
                if p.eat_keyword("EXACT") || p.eat_keyword("CONDITIONAL") {
                } else if p.eat_keyword("APPROXIMATE") {
                    approximate = true;
                } else if p.eat_keyword("PAIRWISE") {
                    command.pairwise = true;
                } else if p.eat_keyword("COMPARE") {
                    command.compare_only = true;
                } else {
                    return Err(p.unexpected(&[
                        "EXACT",
                        "CONDITIONAL",
                        "APPROXIMATE",
                        "PAIRWISE",
                        "COMPARE",
                    ]));
                }
                p.eat(Punct::Comma);
            }
            command.approximate = approximate;
        } else if p.eat_keyword("MISSING") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                if p.eat_keyword("GROUPWISE") {
                    command.listwise = false;
                } else if p.eat_keyword("LISTWISE") {
                    command.listwise = true;
                } else if p.eat_keyword("INCLUDE") {
                    command.include = true;
                } else {
                    return Err(p.unexpected(&["GROUPWISE", "LISTWISE", "INCLUDE"]));
                }
                p.eat(Punct::Comma);
            }
        } else if p.eat_keyword("WRITE") {
            p.eat(Punct::Eq);
            if !p.eat_keyword("NONE") {
                // shortcut: writing the tables needs PROCEDURE OUTPUT, which does not exist yet
                return Err(SyntaxError::new(
                    p.span(),
                    "WRITE=TABLES and WRITE=BOTH of SURVIVAL are not supported yet; they need \
                     PROCEDURE OUTPUT",
                ));
            }
        } else {
            return Err(p.subcommand_error("SURVIVAL", &[]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    let missing = |what: &str| SyntaxError::new(p.span(), format!("SURVIVAL needs {what}"));
    if command.intervals.is_empty() {
        return Err(missing("INTERVAL: THRU end BY width"));
    }
    if command.status.is_empty() {
        return Err(missing("STATUS: the status variable and its event codes"));
    }
    if command.compare_only && command.compare.is_none() {
        return Err(missing("COMPARE when CALCULATE asks for comparisons only"));
    }
    if !calculate || command.compare.is_none() {
        // CALCULATE without COMPARE changes nothing
        (command.approximate, command.pairwise) = (false, false);
    }
    Ok(Command::Survival(Box::new(command)))
}

const ALL_PLOTS: [LifePlot; 5] = [
    LifePlot::Survival,
    LifePlot::LogSurvival,
    LifePlot::Hazard,
    LifePlot::Density,
    LifePlot::OneMinusSurvival,
];
