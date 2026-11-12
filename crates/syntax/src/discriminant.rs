//! Parser of DISCRIMINANT (linear discriminant analysis).
//!
//! Rules from the Command Syntax Reference:
//! - GROUPS=var(min,max), VARIABLES (abbreviated VARS) and SELECT=var(value) come first, in any
//!   order, each once.
//! - Then analysis blocks: ANALYSIS (optional for the first block), METHOD, TOLERANCE,
//!   MAXSTEPS, FIN, FOUT, PIN, POUT, VIN, FUNCTIONS, PRIORS and SAVE; each applies to its own
//!   block and the last of each counts. Every new block starts with ANALYSIS.
//! - Then the global subcommands MISSING, MATRIX, HISTORY, ROTATE, CLASSIFY, STATISTICS and
//!   PLOT. A block subcommand after a global one is ignored with a warning.
//! - ANALYSIS=varlist[(level)] ...: inclusion levels 0 to 99, 1 by default.
//! - FIN 3.84 and FOUT 2.71 by default; PIN and POUT take their place when given. TOLERANCE
//!   0.001, VIN 0. FUNCTIONS=n1[,n2[,n3]] keeps the earlier n2 and n3 when left out.
//! - PRIORS=EQUAL (default) | SIZE | values, `n*c` repeating c n times.
//! - SAVE=CLASS[=name] SCORES[=root] PROBS[=root] (also in parentheses).
//! - STATISTICS without keywords is MEAN STDDEV UNIVF. HISTORY=STEP (alias END) | NONE (aliases
//!   NOSTEP, NOEND). CLASSIFY=NONMISSING | UNSELECTED | UNCLASSIFIED, POOLED | SEPARATE, MEANSUB.

use crate::ast::{Command, VarItem};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::{at_end, keyword_error};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscriminantMethod {
    Direct,
    Wilks,
    Mahal,
    MaxMinF,
    MinResid,
    Rao,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Priors {
    Equal,
    Size,
    Values(Vec<f64>),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct DiscriminantSave {
    /// Each kind asked for, with its name or root name if given.
    pub class: Option<Option<String>>,
    pub scores: Option<Option<String>>,
    pub probs: Option<Option<String>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiscriminantAnalysis {
    /// The variables with their inclusion levels; None is all of VARIABLES at level 1.
    pub variables: Option<Vec<(Vec<VarItem>, u32)>>,
    pub method: DiscriminantMethod,
    pub tolerance: f64,
    pub fin: f64,
    pub fout: f64,
    pub pin: Option<f64>,
    pub pout: Option<f64>,
    pub vin: f64,
    pub max_steps: Option<usize>,
    /// FUNCTIONS: the largest number, the cumulative percentage and the significance.
    pub functions: (Option<usize>, f64, f64),
    pub priors: Priors,
    pub save: Option<DiscriminantSave>,
}

impl Default for DiscriminantAnalysis {
    fn default() -> Self {
        DiscriminantAnalysis {
            variables: None,
            method: DiscriminantMethod::Direct,
            tolerance: 0.001,
            fin: 3.84,
            fout: 2.71,
            pin: None,
            pout: None,
            vin: 0.0,
            max_steps: None,
            functions: (None, 100.0, 1.0),
            priors: Priors::Equal,
            save: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DiscriminantStatistics {
    pub mean: bool,
    pub stddev: bool,
    pub univf: bool,
    pub cov: bool,
    pub corr: bool,
    pub fpair: bool,
    pub boxm: bool,
    pub gcov: bool,
    pub tcov: bool,
    pub raw: bool,
    pub coeff: bool,
    pub table: bool,
    pub crossvalid: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassifyCases {
    NonMissing,
    Unselected,
    Unclassified,
}

/// PLOT: without keywords COMBINED and CASES; ALL is every one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DiscriminantPlots {
    pub combined: bool,
    pub separate: bool,
    pub map: bool,
    /// `CASES[(n)]`: Some(None) lists every case, Some(Some(n)) the first n.
    pub cases: Option<Option<usize>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Discriminant {
    pub group: String,
    pub min: f64,
    pub max: f64,
    pub variables: Vec<VarItem>,
    /// `/SELECT=var(value)`
    pub select: Option<(String, f64)>,
    pub analyses: Vec<DiscriminantAnalysis>,
    pub include: bool,
    pub history: bool,
    pub classify: ClassifyCases,
    pub separate: bool,
    pub meansub: bool,
    pub statistics: DiscriminantStatistics,
    /// PLOT: the charts, the territorial map and the casewise listing asked for.
    pub plot: DiscriminantPlots,
    /// Block subcommands given after a global subcommand, for a warning.
    pub ignored: Vec<String>,
}

const BLOCK: [&str; 12] = [
    "ANALYSIS",
    "METHOD",
    "TOLERANCE",
    "MAXSTEPS",
    "FIN",
    "FOUT",
    "PIN",
    "POUT",
    "VIN",
    "FUNCTIONS",
    "PRIORS",
    "SAVE",
];

fn number(p: &mut Parser, what: &str) -> PResult<(f64, usize)> {
    p.eat(Punct::Eq);
    let start = p.pos;
    let Some(n) = p.number() else {
        return Err(p.unexpected(&[what]));
    };
    Ok((n, start))
}

fn check(p: &Parser, start: usize, ok: bool, message: String) -> PResult<()> {
    match ok {
        true => Ok(()),
        false => Err(SyntaxError::new(p.span_from(start), message)),
    }
}

fn probability(p: &mut Parser, what: &str) -> PResult<f64> {
    let (n, start) = number(p, "a probability")?;
    check(
        p,
        start,
        (0.0..=1.0).contains(&n),
        format!("{what} must lie between 0 and 1, got {n}"),
    )?;
    Ok(n)
}

fn f_value(p: &mut Parser) -> PResult<f64> {
    let (n, start) = number(p, "an F value")?;
    check(
        p,
        start,
        n >= 0.0,
        format!("an F value cannot be negative, got {n}"),
    )?;
    Ok(n)
}

/// `varlist[(level)] [varlist[(level)] ...]`
fn analysis(p: &mut Parser) -> PResult<Vec<(Vec<VarItem>, u32)>> {
    p.eat(Punct::Eq);
    let mut out = Vec::new();
    while matches!(p.peek(), Some(TokenKind::Word(_))) {
        let mut items = Vec::new();
        while matches!(p.peek(), Some(TokenKind::Word(_))) {
            if p.eat_word("ALL") {
                items.push(VarItem::All);
            } else {
                let first = p.name("a variable name")?;
                if p.eat_word("TO") {
                    items.push(VarItem::Range(first, p.name("a variable name")?));
                } else {
                    items.push(VarItem::Name(first));
                }
            }
            p.eat(Punct::Comma);
        }
        let mut level = 1;
        if p.eat(Punct::LParen) {
            let start = p.pos;
            let n = p
                .number()
                .ok_or_else(|| p.unexpected(&["an inclusion level"]))?;
            check(
                p,
                start,
                (0.0..=99.0).contains(&n) && n.fract() == 0.0,
                format!("an inclusion level is a whole number from 0 to 99, got {n}"),
            )?;
            level = n as u32;
            p.expect(Punct::RParen)?;
            p.eat(Punct::Comma);
        }
        out.push((items, level));
    }
    if out.is_empty() {
        return Err(p.unexpected(&["a variable name"]));
    }
    Ok(out)
}

fn priors(p: &mut Parser) -> PResult<Priors> {
    p.eat(Punct::Eq);
    if p.eat_keyword("EQUAL") {
        return Ok(Priors::Equal);
    }
    if p.eat_keyword("SIZE") {
        return Ok(Priors::Size);
    }
    let mut values = Vec::new();
    while !at_end(p) {
        let start = p.pos;
        let Some(v) = p.number() else {
            return Err(p.unexpected(&["EQUAL", "SIZE", "a prior probability"]));
        };
        if p.eat(Punct::Star) {
            check(
                p,
                start,
                v >= 1.0 && v.fract() == 0.0,
                format!("a repeat count must be a positive whole number, got {v}"),
            )?;
            let Some(c) = p.number() else {
                return Err(p.unexpected(&["a prior probability"]));
            };
            values.extend(std::iter::repeat_n(c, v as usize));
        } else {
            values.push(v);
        }
        p.eat(Punct::Comma);
    }
    if values.is_empty() {
        return Err(p.unexpected(&["EQUAL", "SIZE", "a prior probability"]));
    }
    if values.iter().any(|v| *v < 0.0) {
        return Err(SyntaxError::new(
            p.span(),
            "a prior probability cannot be negative",
        ));
    }
    Ok(Priors::Values(values))
}

fn save(p: &mut Parser) -> PResult<DiscriminantSave> {
    p.eat(Punct::Eq);
    let mut save = DiscriminantSave::default();
    while !at_end(p) {
        let slot = if p.eat_keyword("CLASS") {
            &mut save.class
        } else if p.eat_keyword("SCORES") {
            &mut save.scores
        } else if p.eat_keyword("PROBS") {
            &mut save.probs
        } else {
            return Err(p.unexpected(&["CLASS", "SCORES", "PROBS"]));
        };
        let name = if p.eat(Punct::Eq) {
            Some(p.name("a variable name")?)
        } else if p.eat(Punct::LParen) {
            let name = p.name("a variable name")?;
            p.expect(Punct::RParen)?;
            Some(name)
        } else {
            None
        };
        *slot = Some(name);
        p.eat(Punct::Comma);
    }
    if save == DiscriminantSave::default() {
        return Err(p.unexpected(&["CLASS", "SCORES", "PROBS"]));
    }
    Ok(save)
}

fn statistics(p: &mut Parser) -> PResult<DiscriminantStatistics> {
    p.eat(Punct::Eq);
    let mut s = DiscriminantStatistics::default();
    if at_end(p) {
        s.mean = true;
        s.stddev = true;
        s.univf = true;
        return Ok(s);
    }
    const KEYWORDS: [&str; 14] = [
        "MEAN",
        "STDDEV",
        "UNIVF",
        "COV",
        "CORR",
        "FPAIR",
        "BOXM",
        "GCOV",
        "TCOV",
        "RAW",
        "COEFF",
        "TABLE",
        "CROSSVALID",
        "ALL",
    ];
    while !at_end(p) {
        // COV and CORR are written in full, since COV would also abbreviate COVARIANCE
        let Some(i) = KEYWORDS.iter().position(|k| match k.len() {
            3 | 4 => matches!(p.peek(), Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case(k)),
            _ => p.at_keyword(k),
        }) else {
            return Err(p.unexpected(&KEYWORDS));
        };
        p.pos += 1;
        let all = i == 13;
        let flags = [
            &mut s.mean,
            &mut s.stddev,
            &mut s.univf,
            &mut s.cov,
            &mut s.corr,
            &mut s.fpair,
            &mut s.boxm,
            &mut s.gcov,
            &mut s.tcov,
            &mut s.raw,
            &mut s.coeff,
            &mut s.table,
            &mut s.crossvalid,
        ];
        for (k, flag) in flags.into_iter().enumerate() {
            if all || k == i {
                *flag = true;
            }
        }
        p.eat(Punct::Comma);
    }
    Ok(s)
}

/// A block subcommand; false when the word is none of them.
fn block(p: &mut Parser, a: &mut DiscriminantAnalysis) -> PResult<bool> {
    if p.eat_keyword("METHOD") {
        p.eat(Punct::Eq);
        const METHODS: [(&str, DiscriminantMethod); 6] = [
            ("DIRECT", DiscriminantMethod::Direct),
            ("WILKS", DiscriminantMethod::Wilks),
            ("MAHAL", DiscriminantMethod::Mahal),
            ("MAXMINF", DiscriminantMethod::MaxMinF),
            ("MINRESID", DiscriminantMethod::MinResid),
            ("RAO", DiscriminantMethod::Rao),
        ];
        let Some((_, method)) = METHODS.iter().find(|(k, _)| p.at_keyword(k)) else {
            let names: Vec<&str> = METHODS.iter().map(|(k, _)| *k).collect();
            return Err(p.unexpected(&names));
        };
        p.pos += 1;
        a.method = *method;
    } else if p.eat_keyword("TOLERANCE") {
        let (n, start) = number(p, "a tolerance")?;
        check(
            p,
            start,
            n > 0.0 && n < 1.0,
            format!("TOLERANCE must lie between 0 and 1, got {n}"),
        )?;
        a.tolerance = n;
    } else if p.eat_keyword("MAXSTEPS") {
        let (n, start) = number(p, "a number of steps")?;
        check(
            p,
            start,
            n >= 1.0 && n.fract() == 0.0,
            format!("MAXSTEPS must be a positive whole number, got {n}"),
        )?;
        a.max_steps = Some(n as usize);
    } else if p.eat_keyword("FIN") {
        a.fin = f_value(p)?;
    } else if p.eat_keyword("FOUT") {
        a.fout = f_value(p)?;
    } else if p.eat_keyword("PIN") {
        a.pin = Some(probability(p, "PIN")?);
    } else if p.eat_keyword("POUT") {
        a.pout = Some(probability(p, "POUT")?);
    } else if p.eat_keyword("VIN") {
        a.vin = number(p, "a value of Rao's V")?.0;
    } else if p.eat_keyword("FUNCTIONS") {
        p.eat(Punct::Eq);
        let start = p.pos;
        let Some(n1) = p.number() else {
            return Err(p.unexpected(&["a number of functions"]));
        };
        check(
            p,
            start,
            n1 >= 1.0 && n1.fract() == 0.0,
            format!("the number of functions must be a positive whole number, got {n1}"),
        )?;
        a.functions.0 = Some(n1 as usize);
        p.eat(Punct::Comma);
        if let Some(n2) = p.number() {
            a.functions.1 = n2;
            p.eat(Punct::Comma);
            if let Some(n3) = p.number() {
                a.functions.2 = n3;
            }
        }
    } else if p.eat_keyword("PRIORS") {
        a.priors = priors(p)?;
    } else if p.eat_keyword("SAVE") {
        a.save = Some(save(p)?);
    } else {
        return Ok(false);
    }
    Ok(true)
}

/// `var(min,max)` of GROUPS.
fn groups(p: &mut Parser) -> PResult<(String, f64, f64)> {
    p.eat(Punct::Eq);
    let name = p.name("a variable name")?;
    p.expect(Punct::LParen)?;
    let start = p.pos;
    let (Some(min), _, Some(max)) = (p.number(), p.eat(Punct::Comma), p.number()) else {
        return Err(p.unexpected(&["the smallest and the largest group value"]));
    };
    check(
        p,
        start,
        min.fract() == 0.0 && max.fract() == 0.0 && min <= max,
        format!(
            "the group range must be two whole numbers, the smaller first, got {min} and {max}"
        ),
    )?;
    p.expect(Punct::RParen)?;
    Ok((name, min, max))
}

pub(crate) fn discriminant(p: &mut Parser) -> PResult<Command> {
    p.eat(Punct::Slash);
    let mut command = Discriminant {
        group: String::new(),
        min: 0.0,
        max: 0.0,
        variables: Vec::new(),
        select: None,
        analyses: Vec::new(),
        include: false,
        history: true,
        classify: ClassifyCases::NonMissing,
        separate: false,
        meansub: false,
        statistics: DiscriminantStatistics::default(),
        plot: DiscriminantPlots::default(),
        ignored: Vec::new(),
    };
    let mut current = DiscriminantAnalysis::default();
    let (mut in_head, mut global) = (true, false);
    let mut first = true;
    loop {
        if !first && !p.eat(Punct::Slash) {
            break;
        }
        first = false;
        if p.peek().is_none() {
            break;
        }
        let start = p.pos;
        if in_head && p.eat_keyword("GROUPS") {
            check(
                p,
                start,
                command.group.is_empty(),
                "GROUPS is given once".into(),
            )?;
            (command.group, command.min, command.max) = groups(p)?;
        } else if in_head && (p.eat_keyword("VARIABLES") || p.eat_word("VARS")) {
            check(
                p,
                start,
                command.variables.is_empty(),
                "VARIABLES is given once".into(),
            )?;
            p.eat(Punct::Eq);
            command.variables = p.var_list()?;
        } else if in_head && p.eat_keyword("SELECT") {
            check(
                p,
                start,
                command.select.is_none(),
                "SELECT is given once".into(),
            )?;
            p.eat(Punct::Eq);
            let name = p.name("a variable name")?;
            p.expect(Punct::LParen)?;
            let Some(value) = p.number() else {
                return Err(p.unexpected(&["a value"]));
            };
            p.expect(Punct::RParen)?;
            command.select = Some((name, value));
        } else if p.at_keyword("GROUPS") || p.at_keyword("VARIABLES") || p.at_keyword("SELECT") {
            return Err(SyntaxError::new(
                p.span(),
                "GROUPS, VARIABLES and SELECT come before the other subcommands",
            ));
        } else if BLOCK.iter().any(|k| p.at_keyword(k)) && global {
            let name = BLOCK
                .iter()
                .find(|k| p.at_keyword(k))
                .expect("found")
                .to_string();
            command.ignored.push(name);
            while !at_end(p) {
                p.pos += 1;
            }
        } else if p.eat_keyword("ANALYSIS") {
            if !in_head {
                command.analyses.push(std::mem::take(&mut current));
                current = DiscriminantAnalysis::default();
            }
            in_head = false;
            current.variables = Some(analysis(p)?);
        } else if block(p, &mut current)? {
            in_head = false;
        } else {
            in_head = false;
            global = true;
            if p.eat_keyword("MISSING") {
                p.eat(Punct::Eq);
                while !at_end(p) {
                    if p.eat_keyword("EXCLUDE") {
                        command.include = false;
                    } else if p.eat_keyword("INCLUDE") {
                        command.include = true;
                    } else {
                        return Err(p.unexpected(&["EXCLUDE", "INCLUDE"]));
                    }
                    p.eat(Punct::Comma);
                }
            } else if p.eat_keyword("HISTORY") {
                p.eat(Punct::Eq);
                if p.eat_keyword("STEP") || p.eat_word("END") {
                    command.history = true;
                } else if p.eat_keyword("NONE") || p.eat_keyword("NOSTEP") || p.eat_keyword("NOEND")
                {
                    command.history = false;
                } else if !at_end(p) {
                    return Err(p.unexpected(&["STEP", "NONE"]));
                }
            } else if p.eat_keyword("CLASSIFY") {
                p.eat(Punct::Eq);
                while !at_end(p) {
                    if p.eat_keyword("NONMISSING") {
                        command.classify = ClassifyCases::NonMissing;
                    } else if p.eat_keyword("UNSELECTED") {
                        command.classify = ClassifyCases::Unselected;
                    } else if p.eat_keyword("UNCLASSIFIED") {
                        command.classify = ClassifyCases::Unclassified;
                    } else if p.eat_keyword("POOLED") {
                        command.separate = false;
                    } else if p.eat_keyword("SEPARATE") {
                        command.separate = true;
                    } else if p.eat_keyword("MEANSUB") {
                        command.meansub = true;
                    } else {
                        return Err(p.unexpected(&[
                            "NONMISSING",
                            "UNSELECTED",
                            "UNCLASSIFIED",
                            "POOLED",
                            "SEPARATE",
                            "MEANSUB",
                        ]));
                    }
                    p.eat(Punct::Comma);
                }
            } else if p.eat_keyword("STATISTICS") {
                let s = statistics(p)?;
                let t = &mut command.statistics;
                // several STATISTICS subcommands add up
                *t = DiscriminantStatistics {
                    mean: t.mean || s.mean,
                    stddev: t.stddev || s.stddev,
                    univf: t.univf || s.univf,
                    cov: t.cov || s.cov,
                    corr: t.corr || s.corr,
                    fpair: t.fpair || s.fpair,
                    boxm: t.boxm || s.boxm,
                    gcov: t.gcov || s.gcov,
                    tcov: t.tcov || s.tcov,
                    raw: t.raw || s.raw,
                    coeff: t.coeff || s.coeff,
                    table: t.table || s.table,
                    crossvalid: t.crossvalid || s.crossvalid,
                };
            } else if p.eat_keyword("ROTATE") {
                p.eat(Punct::Eq);
                if !p.eat_keyword("NONE") {
                    return Err(keyword_error(
                        p,
                        "DISCRIMINANT /ROTATE",
                        &["COEFF", "STRUCTURE"],
                        &["NONE"],
                    ));
                }
            } else if p.eat_keyword("PLOT") {
                p.eat(Punct::Eq);
                let plot = &mut command.plot;
                if at_end(p) {
                    (plot.combined, plot.cases) = (true, Some(None));
                }
                while !at_end(p) {
                    if p.eat_keyword("COMBINED") {
                        plot.combined = true;
                    } else if p.eat_keyword("SEPARATE") {
                        plot.separate = true;
                    } else if p.eat_keyword("MAP") {
                        plot.map = true;
                    } else if p.eat_keyword("ALL") {
                        (plot.combined, plot.separate, plot.map) = (true, true, true);
                        plot.cases = Some(None);
                    } else if p.eat_keyword("CASES") {
                        plot.cases = Some(None);
                        if p.eat(Punct::LParen) {
                            let start = p.pos;
                            match p.number() {
                                Some(n) if n >= 1.0 && n.fract() == 0.0 => {
                                    plot.cases = Some(Some(n as usize));
                                }
                                _ => {
                                    return Err(SyntaxError::new(
                                        p.span_from(start),
                                        "CASES needs a positive whole number of cases",
                                    ));
                                }
                            }
                            p.expect(Punct::RParen)?;
                        }
                    } else {
                        return Err(p.unexpected(&["COMBINED", "SEPARATE", "MAP", "CASES", "ALL"]));
                    }
                }
            } else {
                return Err(p.subcommand_error("DISCRIMINANT", &["MATRIX", "OUTFILE"]));
            }
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    for (missing, what) in [
        (command.group.is_empty(), "GROUPS=var(min,max)"),
        (command.variables.is_empty(), "VARIABLES=varlist"),
    ] {
        if missing {
            return Err(SyntaxError::new(
                p.span(),
                format!("DISCRIMINANT needs {what}"),
            ));
        }
    }
    command.analyses.push(current);
    Ok(Command::Discriminant(Box::new(command)))
}
