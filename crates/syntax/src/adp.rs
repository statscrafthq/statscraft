//! Parser of ADP: automated preparation of a target and its inputs for modelling.
//!
//! Rules from the Command Syntax Reference (ADP):
//! - FIELDS (with INPUT) and OUTFILE are required; the others are optional and in any order.
//! - Every word in full, each subcommand once, each keyword once within its subcommand, the
//!   equals signs and parentheses as charted, no empty subcommands.
//! - FIELDS [TARGET=var] INPUT=varlist [ANALYSISWEIGHT=var].
//! - PREPDATETIME DATEDURATION=YES[(REFERENCE=CURRENT | YMD('y-m-d') UNIT=AUTO | YEARS | MONTHS
//!   | DAYS [(SUFFIX='s')])] | NO, TIMEDURATION likewise with HMS('h:m:s') and HOURS, MINUTES,
//!   SECONDS; EXTRACTYEAR, EXTRACTMONTH, EXTRACTDAY, EXTRACTHOUR, EXTRACTMINUTE and
//!   EXTRACTSECOND=YES[(SUFFIX='s')] | NO. Durations default to YES, extractions to NO.
//! - SCREENING PCTMISSING=YES[(MAXPCT=50)] | NO, UNIQUECAT=YES[(MAXCAT=100)] | NO,
//!   SINGLECAT=YES[(MAXPCT=95)] | NO (the last off by default); percentages from 0 to 100.
//! - ADJUSTLEVEL INPUT, TARGET=YES | NO, MAXVALORDINAL=10, MINVALCONTINUOUS=5 (positive whole
//!   numbers, MINVALCONTINUOUS at most MAXVALORDINAL).
//! - OUTLIERHANDLING INPUT, TARGET=YES | NO, CUTOFF=SD(3) (positive),
//!   REPLACEWITH=CUTOFFVALUE | MISSING.
//! - REPLACEMISSING INPUT=YES[(EXCLUDE(levels))] | NO (default YES), TARGET likewise (default
//!   NO); the levels CONTINUOUS, NOMINAL and ORDINAL.
//! - REORDERNOMINAL INPUT, TARGET=YES | NO (both NO by default).
//! - RESCALE INPUT=ZSCORE[(MEAN=0 SD=1)] | MINMAX[(MIN=0 MAX=100)] | NONE, TARGET=BOXCOX[(MEAN=0
//!   SD=1)] | NONE; SD positive, MAX above MIN.
//! - TRANSFORM MERGESUPERVISED=YES[(PVALUE=0.05)] | NO, MERGEUNSUPERVISED=YES[([ORDINAL]
//!   [NOMINAL] [MINPCT=10])] | NO (default NO), BINNING=SUPERVISED[(PVALUE=0.05)] | NONE,
//!   SELECTION=YES[(PVALUE=0.05)] | NO, CONSTRUCTION=YES[(ROOT=name)] | NO (default NO);
//!   PVALUE above 0 and at most 1.
//! - CRITERIA SUFFIX(TARGET='_transformed' INPUT='_transformed').
//! - OUTFILE PREPXML='file'.

use crate::ast::{Command, VarItem};
use crate::error::SyntaxError;
use crate::lexer::Punct;
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;
use crate::twostep::{keyword, names_before_keyword, number_from, whole};

/// Measurement levels, as the EXCLUDE lists of REPLACEMISSING name them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AdpLevels {
    pub continuous: bool,
    pub nominal: bool,
    pub ordinal: bool,
}

/// DATEDURATION or TIMEDURATION.
#[derive(Debug, Clone, PartialEq)]
pub struct AdpDuration {
    /// YMD or HMS as its three numbers; None for CURRENT.
    pub reference: Option<[f64; 3]>,
    /// None for AUTO; Some(0..3) for years, months, days (or hours, minutes, seconds).
    pub unit: Option<usize>,
    /// SUFFIX of a fixed unit; None for the unit's own suffix.
    pub suffix: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AdpRescale {
    None,
    Zscore { mean: f64, sd: f64 },
    MinMax { min: f64, max: f64 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Adp {
    pub target: Option<String>,
    pub inputs: Vec<VarItem>,
    pub analysis_weight: Option<String>,
    /// None for NO.
    pub date_duration: Option<AdpDuration>,
    pub time_duration: Option<AdpDuration>,
    /// The suffixes of EXTRACTYEAR, MONTH, DAY, HOUR, MINUTE and SECOND; None for NO.
    pub extract: [Option<String>; 6],
    /// PCTMISSING, UNIQUECAT and SINGLECAT limits; None for NO.
    pub max_missing: Option<f64>,
    pub max_categories: Option<usize>,
    pub max_single: Option<f64>,
    pub adjust_input: bool,
    pub adjust_target: bool,
    pub max_ordinal: usize,
    pub min_continuous: usize,
    pub outlier_input: bool,
    pub outlier_target: bool,
    pub cutoff: f64,
    pub outlier_to_missing: bool,
    /// The levels whose missing values are replaced; None for NO.
    pub replace_input: Option<AdpLevels>,
    pub replace_target: Option<AdpLevels>,
    pub reorder_input: bool,
    pub reorder_target: bool,
    pub rescale_input: AdpRescale,
    /// BOXCOX (mean, sd); None for NONE.
    pub rescale_target: Option<(f64, f64)>,
    /// PVALUE of each; None for NO (NONE).
    pub merge_supervised: Option<f64>,
    /// The levels merged and MINPCT; None for NO.
    pub merge_unsupervised: Option<(AdpLevels, f64)>,
    pub binning: Option<f64>,
    pub selection: Option<f64>,
    pub suffix_target: String,
    pub suffix_input: String,
    pub outfile: String,
}

pub(crate) const ADP_UNITS: [[&str; 3]; 2] =
    [["YEARS", "MONTHS", "DAYS"], ["HOURS", "MINUTES", "SECONDS"]];
pub(crate) const ADP_EXTRACT: [&str; 6] = ["YEAR", "MONTH", "DAY", "HOUR", "MINUTE", "SECOND"];

/// One of `choices` after '='.
fn choice(p: &mut Parser, choices: &[&'static str]) -> PResult<&'static str> {
    p.expect(Punct::Eq)?;
    match choices.iter().find(|c| p.eat_word(c)) {
        Some(c) => Ok(c),
        None => Err(p.unexpected(choices)),
    }
}

fn yes(p: &mut Parser) -> PResult<bool> {
    Ok(choice(p, &["YES", "NO"])? == "YES")
}

/// Settings in parentheses after a choice: each `NAME=` once, read by `item`; nothing without
/// a parenthesis.
fn settings(
    p: &mut Parser,
    names: &[&'static str],
    mut item: impl FnMut(&mut Parser, &'static str) -> PResult<()>,
) -> PResult<()> {
    if !p.eat(Punct::LParen) {
        return Ok(());
    }
    let mut seen = Vec::new();
    while !p.eat(Punct::RParen) {
        let name = keyword(p, &mut seen, names)?;
        item(p, name)?;
    }
    Ok(())
}

/// `=number` from `low` to `high`.
fn value(p: &mut Parser, what: &str, low: f64, high: f64) -> PResult<f64> {
    p.expect(Punct::Eq)?;
    number_from(p, what, low, high)
}

/// `=number` above 0 and at most 1.
fn pvalue(p: &mut Parser) -> PResult<f64> {
    let start = p.pos;
    let v = value(p, "PVALUE", 0.0, 1.0)?;
    if v == 0.0 {
        return Err(SyntaxError::new(
            p.span_from(start),
            "PVALUE must be above 0",
        ));
    }
    Ok(v)
}

/// `YES[(PVALUE=v)]` or the "no" word: Some(v) or None.
fn yes_pvalue(p: &mut Parser, on: &'static str, off: &'static str) -> PResult<Option<f64>> {
    if choice(p, &[on, off])? == off {
        return Ok(None);
    }
    let mut v = 0.05;
    settings(p, &["PVALUE"], |p, _| {
        v = pvalue(p)?;
        Ok(())
    })?;
    Ok(Some(v))
}

/// `(SUFFIX='text')`, optional.
fn suffix(p: &mut Parser) -> PResult<Option<String>> {
    let mut text = None;
    settings(p, &["SUFFIX"], |p, _| {
        p.expect(Punct::Eq)?;
        text = Some(p.string("a quoted suffix")?);
        Ok(())
    })?;
    Ok(text)
}

/// A quoted date or time: three whole numbers separated by any other characters.
fn three_numbers(p: &mut Parser, what: &str) -> PResult<[f64; 3]> {
    p.expect(Punct::LParen)?;
    let start = p.pos;
    let text = p.string(what)?;
    let parts: Vec<f64> = text
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect();
    let Ok(parts) = <[f64; 3]>::try_from(parts) else {
        return Err(SyntaxError::new(
            p.span_from(start),
            format!("{what} needs three numbers in quotes, for example '2024-06-30' or '12:30:00'"),
        ));
    };
    p.expect(Punct::RParen)?;
    Ok(parts)
}

/// DATEDURATION (`kind` 0) or TIMEDURATION (1).
fn duration(p: &mut Parser, kind: usize) -> PResult<Option<AdpDuration>> {
    if !yes(p)? {
        return Ok(None);
    }
    let mut d = AdpDuration {
        reference: None,
        unit: None,
        suffix: None,
    };
    let (reference, what) = match kind {
        0 => ("YMD", "a date"),
        _ => ("HMS", "a time"),
    };
    settings(p, &["REFERENCE", "UNIT"], |p, name| {
        if name == "REFERENCE" {
            if choice(p, &["CURRENT", reference])? != "CURRENT" {
                d.reference = Some(three_numbers(p, what)?);
            }
        } else {
            let units = ADP_UNITS[kind];
            let unit = choice(p, &["AUTO", units[0], units[1], units[2]])?;
            d.unit = units.iter().position(|u| *u == unit);
            if d.unit.is_some() {
                d.suffix = suffix(p)?;
            }
        }
        Ok(())
    })?;
    Ok(Some(d))
}

/// `YES[(EXCLUDE([CONTINUOUS] [NOMINAL] [ORDINAL]))]` or NO: the levels replaced.
fn replace(p: &mut Parser) -> PResult<Option<AdpLevels>> {
    if !yes(p)? {
        return Ok(None);
    }
    let mut levels = AdpLevels {
        continuous: true,
        nominal: true,
        ordinal: true,
    };
    settings(p, &["EXCLUDE"], |p, _| {
        p.expect(Punct::LParen)?;
        let mut seen = Vec::new();
        while !p.eat(Punct::RParen) {
            match keyword(p, &mut seen, &["CONTINUOUS", "NOMINAL", "ORDINAL"])? {
                "CONTINUOUS" => levels.continuous = false,
                "NOMINAL" => levels.nominal = false,
                _ => levels.ordinal = false,
            }
        }
        Ok(())
    })?;
    Ok(Some(levels))
}

/// `NAME(value)` as in CUTOFF=SD(3).
fn wrapped(p: &mut Parser, word: &'static str, what: &str) -> PResult<f64> {
    p.expect(Punct::Eq)?;
    if !p.eat_word(word) {
        return Err(p.unexpected(&[word]));
    }
    p.expect(Punct::LParen)?;
    let v = positive(p, what)?;
    p.expect(Punct::RParen)?;
    Ok(v)
}

fn positive(p: &mut Parser, what: &str) -> PResult<f64> {
    let start = p.pos;
    match p.number() {
        Some(n) if n > 0.0 => Ok(n),
        Some(n) => Err(SyntaxError::new(
            p.span_from(start),
            format!("{what} must be above 0, got {n}"),
        )),
        None => Err(p.unexpected(&[what])),
    }
}

/// MEAN= and SD= in parentheses, defaults 0 and 1.
fn mean_sd(p: &mut Parser) -> PResult<(f64, f64)> {
    let (mut mean, mut sd) = (0.0, 1.0);
    settings(p, &["MEAN", "SD"], |p, name| {
        p.expect(Punct::Eq)?;
        match name {
            "MEAN" => mean = number_from(p, "MEAN", f64::MIN, f64::MAX)?,
            _ => sd = positive(p, "SD")?,
        }
        Ok(())
    })?;
    Ok((mean, sd))
}

pub(crate) fn adp(p: &mut Parser) -> PResult<Command> {
    let transformed = "_transformed".to_string();
    let mut c = Adp {
        target: None,
        inputs: Vec::new(),
        analysis_weight: None,
        date_duration: Some(AdpDuration {
            reference: None,
            unit: None,
            suffix: None,
        }),
        time_duration: Some(AdpDuration {
            reference: None,
            unit: None,
            suffix: None,
        }),
        extract: Default::default(),
        max_missing: Some(50.0),
        max_categories: Some(100),
        max_single: None,
        adjust_input: true,
        adjust_target: true,
        max_ordinal: 10,
        min_continuous: 5,
        outlier_input: true,
        outlier_target: true,
        cutoff: 3.0,
        outlier_to_missing: false,
        replace_input: Some(AdpLevels {
            continuous: true,
            nominal: true,
            ordinal: true,
        }),
        replace_target: None,
        reorder_input: false,
        reorder_target: false,
        rescale_input: AdpRescale::Zscore { mean: 0.0, sd: 1.0 },
        rescale_target: Some((0.0, 1.0)),
        merge_supervised: Some(0.05),
        merge_unsupervised: None,
        binning: Some(0.05),
        selection: Some(0.05),
        suffix_target: transformed.clone(),
        suffix_input: transformed,
        outfile: String::new(),
    };
    let mut subcommands: Vec<&'static str> = Vec::new();
    while p.eat(Punct::Slash) {
        let start = p.pos;
        let names = [
            "FIELDS",
            "PREPDATETIME",
            "SCREENING",
            "ADJUSTLEVEL",
            "OUTLIERHANDLING",
            "REPLACEMISSING",
            "REORDERNOMINAL",
            "RESCALE",
            "TRANSFORM",
            "CRITERIA",
            "OUTFILE",
        ];
        let Some(&sub) = names.iter().find(|n| p.eat_word(n)) else {
            return Err(p.subcommand_error("ADP", &[]));
        };
        if subcommands.contains(&sub) {
            return Err(SyntaxError::new(
                p.span_from(start),
                format!("ADP takes /{sub} once"),
            ));
        }
        subcommands.push(sub);
        if at_end(p) {
            return Err(SyntaxError::new(
                p.span_from(start),
                format!("/{sub} cannot be empty"),
            ));
        }
        let mut seen = Vec::new();
        while !at_end(p) {
            match sub {
                "FIELDS" => match keyword(p, &mut seen, &["TARGET", "INPUT", "ANALYSISWEIGHT"])? {
                    "TARGET" => {
                        p.expect(Punct::Eq)?;
                        c.target = Some(p.name("a variable name")?);
                    }
                    "INPUT" => {
                        p.expect(Punct::Eq)?;
                        c.inputs = names_before_keyword(p)?;
                    }
                    _ => {
                        p.expect(Punct::Eq)?;
                        c.analysis_weight = Some(p.name("a variable name")?);
                    }
                },
                "PREPDATETIME" => {
                    let names = [
                        "DATEDURATION",
                        "TIMEDURATION",
                        "EXTRACTYEAR",
                        "EXTRACTMONTH",
                        "EXTRACTDAY",
                        "EXTRACTHOUR",
                        "EXTRACTMINUTE",
                        "EXTRACTSECOND",
                    ];
                    match keyword(p, &mut seen, &names)? {
                        "DATEDURATION" => c.date_duration = duration(p, 0)?,
                        "TIMEDURATION" => c.time_duration = duration(p, 1)?,
                        name => {
                            let part = &name["EXTRACT".len()..];
                            let i = ADP_EXTRACT.iter().position(|e| *e == part).expect("listed");
                            c.extract[i] = match yes(p)? {
                                true => Some(
                                    suffix(p)?
                                        .unwrap_or_else(|| format!("_{}", part.to_lowercase())),
                                ),
                                false => None,
                            };
                        }
                    }
                }
                "SCREENING" => {
                    let name = keyword(p, &mut seen, &["PCTMISSING", "UNIQUECAT", "SINGLECAT"])?;
                    let on = yes(p)?;
                    let mut limit = match name {
                        "PCTMISSING" => 50.0,
                        "UNIQUECAT" => 100.0,
                        _ => 95.0,
                    };
                    let setting = match name {
                        "UNIQUECAT" => "MAXCAT",
                        _ => "MAXPCT",
                    };
                    if on {
                        settings(p, &[setting], |p, _| {
                            p.expect(Punct::Eq)?;
                            limit = match setting {
                                "MAXCAT" => whole(p, "MAXCAT", 1)? as f64,
                                _ => number_from(p, "MAXPCT", 0.0, 100.0)?,
                            };
                            Ok(())
                        })?;
                    }
                    let limit = on.then_some(limit);
                    match name {
                        "PCTMISSING" => c.max_missing = limit,
                        "UNIQUECAT" => c.max_categories = limit.map(|l| l as usize),
                        _ => c.max_single = limit,
                    }
                }
                "ADJUSTLEVEL" => {
                    let names = ["INPUT", "TARGET", "MAXVALORDINAL", "MINVALCONTINUOUS"];
                    match keyword(p, &mut seen, &names)? {
                        "INPUT" => c.adjust_input = yes(p)?,
                        "TARGET" => c.adjust_target = yes(p)?,
                        name => {
                            p.expect(Punct::Eq)?;
                            let n = whole(p, name, 1)?;
                            match name {
                                "MAXVALORDINAL" => c.max_ordinal = n,
                                _ => c.min_continuous = n,
                            }
                        }
                    }
                }
                "OUTLIERHANDLING" => {
                    let names = ["INPUT", "TARGET", "CUTOFF", "REPLACEWITH"];
                    match keyword(p, &mut seen, &names)? {
                        "INPUT" => c.outlier_input = yes(p)?,
                        "TARGET" => c.outlier_target = yes(p)?,
                        "CUTOFF" => c.cutoff = wrapped(p, "SD", "the cutoff")?,
                        _ => {
                            c.outlier_to_missing =
                                choice(p, &["CUTOFFVALUE", "MISSING"])? == "MISSING"
                        }
                    }
                }
                "REPLACEMISSING" => match keyword(p, &mut seen, &["INPUT", "TARGET"])? {
                    "INPUT" => c.replace_input = replace(p)?,
                    _ => c.replace_target = replace(p)?,
                },
                "REORDERNOMINAL" => match keyword(p, &mut seen, &["INPUT", "TARGET"])? {
                    "INPUT" => c.reorder_input = yes(p)?,
                    _ => c.reorder_target = yes(p)?,
                },
                "RESCALE" => match keyword(p, &mut seen, &["INPUT", "TARGET"])? {
                    "INPUT" => {
                        c.rescale_input = match choice(p, &["ZSCORE", "MINMAX", "NONE"])? {
                            "ZSCORE" => {
                                let (mean, sd) = mean_sd(p)?;
                                AdpRescale::Zscore { mean, sd }
                            }
                            "MINMAX" => {
                                let at = p.pos;
                                let (mut min, mut max) = (0.0, 100.0);
                                settings(p, &["MIN", "MAX"], |p, name| {
                                    let v = value(p, name, f64::MIN, f64::MAX)?;
                                    match name {
                                        "MIN" => min = v,
                                        _ => max = v,
                                    }
                                    Ok(())
                                })?;
                                if max <= min {
                                    return Err(SyntaxError::new(
                                        p.span_from(at),
                                        "MINMAX needs MAX above MIN",
                                    ));
                                }
                                AdpRescale::MinMax { min, max }
                            }
                            _ => AdpRescale::None,
                        }
                    }
                    _ => {
                        c.rescale_target = match choice(p, &["BOXCOX", "NONE"])? {
                            "BOXCOX" => Some(mean_sd(p)?),
                            _ => None,
                        }
                    }
                },
                "TRANSFORM" => {
                    let names = [
                        "MERGESUPERVISED",
                        "MERGEUNSUPERVISED",
                        "BINNING",
                        "SELECTION",
                        "CONSTRUCTION",
                    ];
                    let at = p.pos;
                    match keyword(p, &mut seen, &names)? {
                        "MERGESUPERVISED" => c.merge_supervised = yes_pvalue(p, "YES", "NO")?,
                        "BINNING" => c.binning = yes_pvalue(p, "SUPERVISED", "NONE")?,
                        "SELECTION" => c.selection = yes_pvalue(p, "YES", "NO")?,
                        "MERGEUNSUPERVISED" => {
                            c.merge_unsupervised = None;
                            if yes(p)? {
                                let mut levels = AdpLevels::default();
                                let mut minpct = 10.0;
                                if p.eat(Punct::LParen) {
                                    let mut seen = Vec::new();
                                    while !p.eat(Punct::RParen) {
                                        match keyword(
                                            p,
                                            &mut seen,
                                            &["ORDINAL", "NOMINAL", "MINPCT"],
                                        )? {
                                            "ORDINAL" => levels.ordinal = true,
                                            "NOMINAL" => levels.nominal = true,
                                            _ => minpct = value(p, "MINPCT", 0.0, 100.0)?,
                                        }
                                    }
                                }
                                c.merge_unsupervised = Some((levels, minpct));
                            }
                        }
                        _ => {
                            if yes(p)? {
                                return Err(SyntaxError::new(
                                    p.span_from(at),
                                    "CONSTRUCTION=YES (new predictors from principal components) is not supported yet",
                                ));
                            }
                        }
                    }
                }
                "CRITERIA" => {
                    keyword(p, &mut seen, &["SUFFIX"])?;
                    p.expect(Punct::LParen)?;
                    let mut roles = Vec::new();
                    while !p.eat(Punct::RParen) {
                        let role = keyword(p, &mut roles, &["TARGET", "INPUT"])?;
                        p.expect(Punct::Eq)?;
                        let text = p.string("a quoted suffix")?;
                        match role {
                            "TARGET" => c.suffix_target = text,
                            _ => c.suffix_input = text,
                        }
                    }
                }
                _ => {
                    keyword(p, &mut seen, &["PREPXML"])?;
                    p.expect(Punct::Eq)?;
                    c.outfile = p.file("a file")?;
                }
            }
        }
        if sub == "FIELDS" && c.inputs.is_empty() {
            return Err(SyntaxError::new(
                p.span_from(start),
                "FIELDS needs INPUT= with the input fields",
            ));
        }
        if sub == "ADJUSTLEVEL" && c.min_continuous > c.max_ordinal {
            return Err(SyntaxError::new(
                p.span_from(start),
                "MINVALCONTINUOUS cannot be above MAXVALORDINAL",
            ));
        }
    }
    p.finish(&["'/'"])?;
    for required in ["FIELDS", "OUTFILE"] {
        if !subcommands.contains(&required) {
            return Err(SyntaxError::new(
                p.span(),
                match required {
                    "FIELDS" => "ADP needs /FIELDS with INPUT= and the input fields",
                    _ => "ADP needs /OUTFILE PREPXML='file' for the preparation syntax",
                },
            ));
        }
    }
    Ok(Command::Adp(Box::new(c)))
}
