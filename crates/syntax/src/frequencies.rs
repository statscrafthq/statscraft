//! Parser of FREQUENCIES: frequency tables, statistics and percentiles of variables.
//!
//! Rules from the Command Syntax Reference (FREQUENCIES):
//! - VARIABLES is the only required subcommand; the others come in any order, may be shortened
//!   to three letters, and their '=' is optional. `/STATS` also names STATISTICS.
//! - FORMAT: NOTABLE, LIMIT(n), and one of AVALUE (default), DVALUE, AFREQ, DFREQ.
//! - STATISTICS alone is DEFAULT (MEAN STDDEV MINIMUM MAXIMUM); NONE drops what came before.
//! - PERCENTILES lists values from 0 to 100; NTILES=n adds the n - 1 cut points; several of
//!   both may be given and duplicates are shown once.
//! - GROUPED=varlist [(width) | (boundaries)], more lists after '/' or in one subcommand.
//! - BARCHART and HISTOGRAM exclude each other; PIECHART goes with either. MIN(n) and MAX(n)
//!   bound the values plotted. Bar: FREQ(n) or PERCENT(n), n the top of the axis. Pie: FREQ or
//!   PERCENT, MISSING (one slice for the missing values, the default) or NOMISSING; of each
//!   pair the first given counts. Histogram: FREQ(n), NORMAL or NONORMAL (the default).

use crate::ast::{
    Command, Frequencies, FrequencyChart, FrequencyChartKind, FrequencyChartKind as Kind,
    FrequencyOrder, FrequencyStatistic as S, Grouping, VarItem,
};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::{at_end, names};
use crate::table::matches_keyword;

const SUBCOMMANDS: [&str; 11] = [
    "VARIABLES",
    "FORMAT",
    "MISSING",
    "STATISTICS",
    "PERCENTILES",
    "NTILES",
    "BARCHART",
    "PIECHART",
    "HISTOGRAM",
    "GROUPED",
    "ORDER",
];

const CHARTS: [(&str, Kind); 3] = [
    ("BARCHART", Kind::Bar),
    ("PIECHART", Kind::Pie),
    ("HISTOGRAM", Kind::Histogram),
];

const ALL: [S; 14] = [
    S::Mean,
    S::SeMean,
    S::Median,
    S::Mode,
    S::StdDev,
    S::Variance,
    S::Skewness,
    S::SeSkewness,
    S::Kurtosis,
    S::SeKurtosis,
    S::Range,
    S::Minimum,
    S::Maximum,
    S::Sum,
];

const DEFAULT: [S; 4] = [S::Mean, S::StdDev, S::Minimum, S::Maximum];

const STATISTICS: [(&str, &[S]); 17] = [
    ("MEAN", &[S::Mean]),
    ("SEMEAN", &[S::SeMean]),
    ("MEDIAN", &[S::Median]),
    ("MODE", &[S::Mode]),
    ("STDDEV", &[S::StdDev]),
    ("VARIANCE", &[S::Variance]),
    ("SKEWNESS", &[S::Skewness]),
    ("SESKEW", &[S::SeSkewness]),
    ("KURTOSIS", &[S::Kurtosis]),
    ("SEKURT", &[S::SeKurtosis]),
    ("RANGE", &[S::Range]),
    ("MINIMUM", &[S::Minimum]),
    ("MAXIMUM", &[S::Maximum]),
    ("SUM", &[S::Sum]),
    ("DEFAULT", &DEFAULT),
    ("ALL", &ALL),
    ("NONE", &[]),
];

/// True when `word` names a subcommand of FREQUENCIES.
fn is_subcommand(word: &str) -> bool {
    word.eq_ignore_ascii_case("STATS") || SUBCOMMANDS.iter().any(|s| matches_keyword(word, s))
}

/// A whole number of at least `least` in parentheses.
fn count_in_parentheses(p: &mut Parser, what: &str, least: f64) -> PResult<f64> {
    p.expect(Punct::LParen)?;
    let start = p.pos;
    match p.number() {
        Some(n) if n >= least && n.fract() == 0.0 => {
            p.expect(Punct::RParen)?;
            Ok(n)
        }
        _ => Err(SyntaxError::new(
            p.span_from(start),
            format!("{what} takes a whole number of at least {least} in parentheses"),
        )),
    }
}

fn format(p: &mut Parser, command: &mut Frequencies) -> PResult<()> {
    const ORDERS: [(&str, FrequencyOrder); 4] = [
        ("AVALUE", FrequencyOrder::AValue),
        ("DVALUE", FrequencyOrder::DValue),
        ("AFREQ", FrequencyOrder::AFreq),
        ("DFREQ", FrequencyOrder::DFreq),
    ];
    let expected = ["NOTABLE", "LIMIT(n)", "AVALUE", "DVALUE", "AFREQ", "DFREQ"];
    if at_end(p) {
        return Err(p.unexpected(&expected));
    }
    while !at_end(p) {
        if p.eat_keyword("NOTABLE") {
            command.notable = true;
        } else if p.eat_keyword("LIMIT") {
            command.limit = Some(count_in_parentheses(p, "LIMIT", 0.0)? as usize);
        } else if let Some((_, order)) = ORDERS.iter().find(|(k, _)| p.at_keyword(k)) {
            p.pos += 1;
            command.order = *order;
        } else {
            return Err(p.unexpected(&expected));
        }
        p.eat(Punct::Comma);
    }
    Ok(())
}

fn statistics(p: &mut Parser, chosen: &mut Vec<S>) -> PResult<()> {
    if at_end(p) {
        chosen.extend(DEFAULT);
        return Ok(());
    }
    while !at_end(p) {
        let Some((name, list)) = STATISTICS.iter().find(|(k, _)| p.at_keyword(k)) else {
            return Err(p.unexpected(&names(&STATISTICS)));
        };
        p.pos += 1;
        if *name == "NONE" {
            chosen.clear();
        }
        chosen.extend_from_slice(list);
        p.eat(Punct::Comma);
    }
    Ok(())
}

fn percentiles(p: &mut Parser, list: &mut Vec<f64>) -> PResult<()> {
    let mut any = false;
    while !at_end(p) {
        let start = p.pos;
        let Some(n) = p.number() else {
            return Err(p.unexpected(&["a number from 0 to 100"]));
        };
        if !(0.0..=100.0).contains(&n) {
            let message = "a percentile must lie between 0 and 100";
            return Err(SyntaxError::new(p.span_from(start), message));
        }
        list.push(n);
        any = true;
        p.eat(Punct::Comma);
    }
    if !any {
        return Err(p.unexpected(&["a number from 0 to 100"]));
    }
    Ok(())
}

fn ntiles(p: &mut Parser, list: &mut Vec<f64>) -> PResult<()> {
    let start = p.pos;
    let n = match p.number() {
        // a bound keeps the n - 1 cut points from filling the memory
        Some(n) if (1.0..=1000.0).contains(&n) && n.fract() == 0.0 => n,
        _ => {
            let message = "NTILES takes a whole number from 1 to 1000";
            return Err(SyntaxError::new(p.span_from(start), message));
        }
    };
    list.extend((1..n as u32).map(|i| 100.0 * f64::from(i) / n));
    Ok(())
}

/// `varlist [(width) | (boundary, ...)]`, repeated, with '/' between lists allowed.
fn grouped(p: &mut Parser, command: &mut Frequencies) -> PResult<()> {
    loop {
        let mut items = Vec::new();
        while matches!(p.peek(), Some(TokenKind::Word(_))) {
            items.push(if p.eat_word("ALL") {
                VarItem::All
            } else {
                let first = p.name("a variable name")?;
                if p.eat_word("TO") {
                    VarItem::Range(first, p.name("a variable name")?)
                } else {
                    VarItem::Name(first)
                }
            });
            p.eat(Punct::Comma);
        }
        if items.is_empty() {
            return Err(p.unexpected(&["a variable name"]));
        }
        let mut grouping = Grouping::Midpoints;
        if p.eat(Punct::LParen) {
            let start = p.pos;
            let mut values = Vec::new();
            while let Some(n) = p.number() {
                values.push(n);
                p.eat(Punct::Comma);
            }
            p.expect(Punct::RParen)?;
            grouping = match values[..] {
                [] => {
                    let message = "give a group width or the group boundaries in parentheses";
                    return Err(SyntaxError::new(p.span_from(start), message));
                }
                [width] if width > 0.0 => Grouping::Width(width),
                [_] => {
                    let message = "the group width must be positive";
                    return Err(SyntaxError::new(p.span_from(start), message));
                }
                _ if values.windows(2).any(|w| w[0] >= w[1]) => {
                    let message = "the group boundaries must be in ascending order";
                    return Err(SyntaxError::new(p.span_from(start), message));
                }
                _ => Grouping::Boundaries(values),
            };
            p.eat(Punct::Comma);
        }
        command.grouped.push((items, grouping));
        if !at_end(p) {
            continue;
        }
        // another variable list may follow a slash
        match p.peek_at(1) {
            Some(TokenKind::Word(w)) if p.at(Punct::Slash) && !is_subcommand(w) => p.pos += 1,
            _ => return Ok(()),
        }
    }
}

/// The options of a chart subcommand, those of `kind` only. Of FREQ and PERCENT, MISSING and
/// NOMISSING, NORMAL and NONORMAL the first given counts; FREQ and PERCENT may take `(n)`.
fn chart(p: &mut Parser, kind: FrequencyChartKind) -> PResult<FrequencyChart> {
    let keywords: &[&str] = match kind {
        Kind::Bar => &["MINIMUM", "MAXIMUM", "FREQ", "PERCENT"],
        Kind::Pie => &[
            "MINIMUM",
            "MAXIMUM",
            "FREQ",
            "PERCENT",
            "MISSING",
            "NOMISSING",
        ],
        Kind::Histogram => &["MINIMUM", "MAXIMUM", "FREQ", "NORMAL", "NONORMAL"],
    };
    let mut chart = FrequencyChart {
        kind,
        minimum: None,
        maximum: None,
        percent: false,
        scale: None,
        missing: true,
        normal: false,
    };
    let (mut scaled, mut missing, mut normal) = (false, false, false);
    while !at_end(p) {
        let Some(keyword) = keywords.iter().find(|k| p.at_keyword(k)) else {
            return Err(p.unexpected(keywords));
        };
        p.pos += 1;
        let number = |p: &mut Parser| -> PResult<f64> {
            p.expect(Punct::LParen)?;
            let n = p.number().ok_or_else(|| p.unexpected(&["a number"]))?;
            p.expect(Punct::RParen)?;
            Ok(n)
        };
        match *keyword {
            "MINIMUM" => chart.minimum = Some(number(p)?),
            "MAXIMUM" => chart.maximum = Some(number(p)?),
            "FREQ" | "PERCENT" => {
                // the pie chart has no axis to scale
                let n = match kind != Kind::Pie && p.at(Punct::LParen) {
                    true => Some(number(p)?),
                    false => None,
                };
                if !std::mem::replace(&mut scaled, true) {
                    chart.percent = *keyword == "PERCENT";
                    chart.scale = n;
                }
            }
            "MISSING" | "NOMISSING" if !std::mem::replace(&mut missing, true) => {
                chart.missing = *keyword == "MISSING"
            }
            "NORMAL" | "NONORMAL" if !std::mem::replace(&mut normal, true) => {
                chart.normal = *keyword == "NORMAL"
            }
            _ => {}
        }
        p.eat(Punct::Comma);
    }
    Ok(chart)
}

pub(crate) fn frequencies(p: &mut Parser) -> PResult<Command> {
    let mut command = Frequencies {
        variables: Vec::new(),
        notable: false,
        limit: None,
        order: FrequencyOrder::AValue,
        include: false,
        statistics: Vec::new(),
        percentiles: Vec::new(),
        grouped: Vec::new(),
        charts: Vec::new(),
        by_variable: false,
    };
    p.eat(Punct::Slash);
    let mut more = true;
    // a word is a subcommand only when '=' follows, so the bare form `FREQUENCIES var.` works
    // even for a variable whose name looks like a keyword
    if p.peek_at(1) != Some(&TokenKind::Punct(Punct::Eq)) {
        if p.eat_word("VARIABLES") {
            return Err(p.unexpected(&["'='"]));
        }
        command.variables = p.var_list()?;
        more = p.eat(Punct::Slash);
    }
    while more {
        let start = p.pos;
        if p.eat_keyword("VARIABLES") {
            p.eat(Punct::Eq);
            command.variables.extend(p.var_list()?);
        } else if p.eat_keyword("FORMAT") {
            p.eat(Punct::Eq);
            format(p, &mut command)?;
        } else if p.eat_keyword("MISSING") {
            p.eat(Punct::Eq);
            if !p.eat_keyword("INCLUDE") {
                return Err(p.unexpected(&["INCLUDE"]));
            }
            command.include = true;
        } else if p.eat_keyword("STATISTICS") || p.eat_word("STATS") {
            p.eat(Punct::Eq);
            statistics(p, &mut command.statistics)?;
        } else if p.eat_keyword("PERCENTILES") {
            p.eat(Punct::Eq);
            percentiles(p, &mut command.percentiles)?;
        } else if p.eat_keyword("NTILES") {
            p.eat(Punct::Eq);
            ntiles(p, &mut command.percentiles)?;
        } else if p.eat_keyword("GROUPED") {
            p.eat(Punct::Eq);
            grouped(p, &mut command)?;
        } else if p.eat_keyword("ORDER") {
            p.eat(Punct::Eq);
            if p.eat_keyword("VARIABLE") {
                command.by_variable = true;
            } else if p.eat_keyword("ANALYSIS") {
                command.by_variable = false;
            } else if !at_end(p) {
                return Err(p.unexpected(&["ANALYSIS", "VARIABLE"]));
            }
        } else if let Some((_, kind)) = CHARTS.iter().find(|(k, _)| p.at_keyword(k)) {
            p.pos += 1;
            p.eat(Punct::Eq);
            let chart = chart(p, *kind)?;
            command.charts.retain(|c| c.kind != chart.kind);
            command.charts.push(chart);
            let has = |kind: Kind| command.charts.iter().any(|c| c.kind == kind);
            if has(Kind::Bar) && has(Kind::Histogram) {
                let message = "BARCHART and HISTOGRAM cannot be used together";
                return Err(SyntaxError::new(p.span_from(start), message));
            }
        } else {
            return Err(p.subcommand_error("FREQUENCIES", &[]));
        }
        more = p.eat(Punct::Slash);
    }
    p.finish(&["'/'"])?;
    if command.variables.is_empty() {
        return Err(p.unexpected(&["VARIABLES=varlist"]));
    }
    command.statistics.sort();
    command.statistics.dedup();
    command.percentiles.sort_by(f64::total_cmp);
    command.percentiles.dedup();
    Ok(Command::Frequencies(command))
}
