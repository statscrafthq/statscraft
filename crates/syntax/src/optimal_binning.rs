//! Parser of OPTIMAL BINNING: bins of scale variables, supervised by a guide variable (MDLP)
//! or by equal frequency.
//!
//! Rules from the Command Syntax Reference (OPTIMAL BINNING):
//! - VARIABLES is required: `[GUIDE=var] BIN=varlist [SAVE=NO | YES [(INTO=new varlist)]]`.
//! - CRITERIA PREPROCESS=EQUALFREQ[(BINS=1000)] | NONE, METHOD=MDLP | EQUALFREQ[(BINS=10)]
//!   (BINS a whole number above 1), LOWEREND and UPPEREND=UNBOUNDED | OBSERVED,
//!   LOWERLIMIT=INCLUSIVE | EXCLUSIVE, FORCEMERGE=value (0, no merging).
//! - METHOD defaults to MDLP with a guide and EQUALFREQ without; MDLP needs a guide.
//! - MISSING SCOPE=PAIRWISE | LISTWISE; OUTFILE RULES=file; PRINT ENDPOINTS (default)
//!   DESCRIPTIVES ENTROPY NONE, NONE alone.
//! - Every word in full, each subcommand once, each keyword once within its subcommand, the
//!   equals signs and parentheses required, an empty subcommand ignored.

use crate::ast::{Command, VarItem};
use crate::error::SyntaxError;
use crate::lexer::Punct;
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;
use crate::twostep::{keyword, names_before_keyword, number_from, whole};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinningMethod {
    Mdlp,
    /// Equal frequency with this many bins at most.
    EqualFreq(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub struct OptimalBinning {
    pub guide: Option<String>,
    pub bin: Vec<VarItem>,
    pub save: bool,
    /// INTO names; empty for the default names.
    pub into: Vec<VarItem>,
    /// PREPROCESS=EQUALFREQ(BINS=n) as Some(n), NONE as None.
    pub preprocess: Option<usize>,
    pub method: BinningMethod,
    pub lower_observed: bool,
    pub upper_observed: bool,
    pub lower_exclusive: bool,
    pub force_merge: f64,
    pub listwise: bool,
    pub rules: Option<String>,
    pub print_endpoints: bool,
    pub print_descriptives: bool,
    pub print_entropy: bool,
}

/// One of `choices` after '='.
fn choice(p: &mut Parser, choices: &[&'static str]) -> PResult<&'static str> {
    p.expect(Punct::Eq)?;
    match choices.iter().find(|c| p.eat_word(c)) {
        Some(c) => Ok(c),
        None => Err(p.unexpected(choices)),
    }
}

/// An optional `(BINS=n)`, n above 1.
fn bins(p: &mut Parser, default: usize) -> PResult<usize> {
    if !p.eat(Punct::LParen) {
        return Ok(default);
    }
    if !p.eat_word("BINS") {
        return Err(p.unexpected(&["BINS"]));
    }
    p.expect(Punct::Eq)?;
    let n = whole(p, "BINS", 2)?;
    p.expect(Punct::RParen)?;
    Ok(n)
}

pub(crate) fn optimal_binning(p: &mut Parser) -> PResult<Command> {
    let mut command = OptimalBinning {
        guide: None,
        bin: Vec::new(),
        save: false,
        into: Vec::new(),
        preprocess: Some(1000),
        method: BinningMethod::Mdlp,
        lower_observed: false,
        upper_observed: false,
        lower_exclusive: false,
        force_merge: 0.0,
        listwise: false,
        rules: None,
        print_endpoints: true,
        print_descriptives: false,
        print_entropy: false,
    };
    let mut method = None;
    let mut subcommands: Vec<&'static str> = Vec::new();
    while p.eat(Punct::Slash) {
        if at_end(p) {
            continue;
        }
        let start = p.pos;
        let names = ["VARIABLES", "CRITERIA", "MISSING", "OUTFILE", "PRINT"];
        let Some(&sub) = names.iter().find(|n| p.eat_word(n)) else {
            return Err(p.subcommand_error("OPTIMAL BINNING", &[]));
        };
        if subcommands.contains(&sub) {
            return Err(SyntaxError::new(
                p.span_from(start),
                format!("OPTIMAL BINNING takes /{sub} once"),
            ));
        }
        if at_end(p) {
            // an empty subcommand is ignored
            continue;
        }
        subcommands.push(sub);
        let mut seen = Vec::new();
        match sub {
            "VARIABLES" => {
                while !at_end(p) {
                    match keyword(p, &mut seen, &["GUIDE", "BIN", "SAVE"])? {
                        "GUIDE" => {
                            p.expect(Punct::Eq)?;
                            command.guide = Some(p.name("a variable name")?);
                        }
                        "BIN" => {
                            p.expect(Punct::Eq)?;
                            command.bin = names_before_keyword(p)?;
                        }
                        _ => {
                            command.save = choice(p, &["YES", "NO"])? == "YES";
                            if command.save && p.eat(Punct::LParen) {
                                if !p.eat_word("INTO") {
                                    return Err(p.unexpected(&["INTO"]));
                                }
                                p.expect(Punct::Eq)?;
                                command.into = names_before_keyword(p)?;
                                p.expect(Punct::RParen)?;
                            }
                        }
                    }
                }
                if command.bin.is_empty() {
                    return Err(SyntaxError::new(
                        p.span_from(start),
                        "VARIABLES needs BIN= with the variables to bin",
                    ));
                }
            }
            "CRITERIA" => {
                let names = [
                    "PREPROCESS",
                    "METHOD",
                    "LOWEREND",
                    "UPPEREND",
                    "LOWERLIMIT",
                    "FORCEMERGE",
                ];
                while !at_end(p) {
                    match keyword(p, &mut seen, &names)? {
                        "PREPROCESS" => {
                            command.preprocess = match choice(p, &["EQUALFREQ", "NONE"])? {
                                "NONE" => None,
                                _ => Some(bins(p, 1000)?),
                            }
                        }
                        "METHOD" => {
                            method = Some(match choice(p, &["MDLP", "EQUALFREQ"])? {
                                "MDLP" => BinningMethod::Mdlp,
                                _ => BinningMethod::EqualFreq(bins(p, 10)?),
                            })
                        }
                        "LOWEREND" => {
                            command.lower_observed =
                                choice(p, &["UNBOUNDED", "OBSERVED"])? == "OBSERVED"
                        }
                        "UPPEREND" => {
                            command.upper_observed =
                                choice(p, &["UNBOUNDED", "OBSERVED"])? == "OBSERVED"
                        }
                        "LOWERLIMIT" => {
                            command.lower_exclusive =
                                choice(p, &["INCLUSIVE", "EXCLUSIVE"])? == "EXCLUSIVE"
                        }
                        _ => {
                            p.expect(Punct::Eq)?;
                            command.force_merge = number_from(p, "FORCEMERGE", 0.0, f64::MAX)?;
                        }
                    }
                }
            }
            "MISSING" => {
                while !at_end(p) {
                    keyword(p, &mut seen, &["SCOPE"])?;
                    command.listwise = choice(p, &["PAIRWISE", "LISTWISE"])? == "LISTWISE";
                }
            }
            "OUTFILE" => {
                while !at_end(p) {
                    keyword(p, &mut seen, &["RULES"])?;
                    p.expect(Punct::Eq)?;
                    command.rules = Some(p.file("a file")?);
                }
            }
            _ => {
                command.print_endpoints = false;
                let names = ["ENDPOINTS", "DESCRIPTIVES", "ENTROPY", "NONE"];
                while !at_end(p) {
                    match keyword(p, &mut seen, &names)? {
                        "ENDPOINTS" => command.print_endpoints = true,
                        "DESCRIPTIVES" => command.print_descriptives = true,
                        "ENTROPY" => command.print_entropy = true,
                        _ => {}
                    }
                }
                if seen.contains(&"NONE") && seen.len() > 1 {
                    return Err(SyntaxError::new(
                        p.span_from(start),
                        "PRINT NONE cannot be given with other keywords",
                    ));
                }
            }
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    if !subcommands.contains(&"VARIABLES") {
        return Err(SyntaxError::new(
            p.span(),
            "OPTIMAL BINNING needs /VARIABLES with BIN= and the variables to bin",
        ));
    }
    command.method = match method {
        Some(m) => m,
        None if command.guide.is_some() => BinningMethod::Mdlp,
        None => BinningMethod::EqualFreq(10),
    };
    if command.method == BinningMethod::Mdlp && command.guide.is_none() {
        return Err(SyntaxError::new(
            p.span(),
            "METHOD=MDLP needs a guide variable: give GUIDE= on /VARIABLES",
        ));
    }
    if command.method != BinningMethod::Mdlp {
        // the guide and the preprocessing only serve MDLP
        command.guide = None;
        command.preprocess = None;
    }
    Ok(Command::OptimalBinning(Box::new(command)))
}
