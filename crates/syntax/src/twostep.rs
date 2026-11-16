//! Parsers of TWOSTEP CLUSTER (with its other name DMCLUSTER) and DETECTANOMALY.
//!
//! TWOSTEP CLUSTER, rules from the Command Syntax Reference:
//! - CATEGORICAL VARIABLES= and CONTINUOUS VARIABLES= name the variables; at least one is
//!   needed. Subcommands come in any order; a repeated subcommand replaces the earlier one and
//!   an empty one is ignored. Keywords may be shortened to three letters.
//! - CRITERIA INITHRESHOLD(0) MXBRANCH(8) MXLEVEL(3); the text also spells INITTHRESHOLD.
//!   DISTANCE LIKELIHOOD (default) or EUCLIDEAN. HANDLENOISE 0 (percent). MEMALLOCATE 64,
//!   at least 4. MISSING EXCLUDE (default) or INCLUDE. NOSTANDARDIZE [VARIABLES=list].
//!   NUMCLUSTERS AUTO [15] [BIC | AIC] or FIXED=n. PRINT IC COUNT SUMMARY. VIEWMODEL
//!   DISPLAY=YES|NO EVALUATIONFIELDS=list. SAVE CLUSTER [VARIABLE=name]. OUTFILE MODEL='file'
//!   STATE='file'. INFILE FILE='file'.
//!
//! DETECTANOMALY: every subcommand is optional and may be given once; keywords are written in
//! full, the equals signs are required and a keyword may appear once within its subcommand.
//! VARIABLES CATEGORICAL= SCALE= ID= EXCEPT=; HANDLEMISSING APPLY=NO CREATEMISPROPVAR=NO;
//! CRITERIA MINNUMPEERS=1 MAXNUMPEERS=15 MLWEIGHT=6 NUMREASONS=1 PCTANOMALOUSCASES=5
//! NUMANOMALOUSCASES=n ANOMALYCUTPOINT=2|NONE; SAVE keyword[(name)] ...; OUTFILE MODEL=file;
//! PRINT CPS ANOMALYLIST NORMS ANOMALYSUMMARY REASONSUMMARY NONE (ANOMALYLIST without PRINT).

use crate::ast::{Command, VarItem};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClusterCount {
    /// Chosen by BIC (or AIC when `aic`) from 1 to `max`.
    Auto {
        max: usize,
        aic: bool,
    },
    Fixed(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Twostep {
    pub categorical: Vec<VarItem>,
    pub continuous: Vec<VarItem>,
    pub threshold: f64,
    pub branch: usize,
    pub levels: usize,
    pub euclidean: bool,
    /// HANDLENOISE, a percent; 0 is no noise handling.
    pub noise: f64,
    pub infile: Option<String>,
    pub memory: usize,
    pub include: bool,
    /// NOSTANDARDIZE VARIABLES=: the continuous variables left as they are.
    pub unstandardized: Vec<VarItem>,
    pub count: ClusterCount,
    pub model_file: Option<String>,
    pub state_file: Option<String>,
    pub print_ic: bool,
    pub print_count: bool,
    pub print_summary: bool,
    pub view: bool,
    pub evaluation: Vec<VarItem>,
    /// SAVE CLUSTER, with the name given by VARIABLE=.
    pub save: Option<Option<String>>,
}

/// Names until a word followed by `=` (the next keyword) or the end of the subcommand.
fn names_before_keyword(p: &mut Parser) -> PResult<Vec<VarItem>> {
    let mut items = Vec::new();
    while let Some(TokenKind::Word(_)) = p.peek() {
        if matches!(p.peek_at(1), Some(TokenKind::Punct(Punct::Eq))) {
            break;
        }
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
    if items.is_empty() {
        return Err(p.unexpected(&["a variable name"]));
    }
    Ok(items)
}

/// A whole number of at least `low` in parentheses, as in MXBRANCH(8).
fn paren_whole(p: &mut Parser, what: &str, low: usize) -> PResult<usize> {
    p.expect(Punct::LParen)?;
    let n = whole(p, what, low)?;
    p.expect(Punct::RParen)?;
    Ok(n)
}

fn whole(p: &mut Parser, what: &str, low: usize) -> PResult<usize> {
    let start = p.pos;
    match p.number() {
        Some(n) if n.fract() == 0.0 && n >= low as f64 && n < 1e9 => Ok(n as usize),
        Some(n) => Err(SyntaxError::new(
            p.span_from(start),
            format!("{what} must be a whole number of at least {low}, got {n}"),
        )),
        None => Err(p.unexpected(&[what])),
    }
}

fn number_from(p: &mut Parser, what: &str, low: f64, high: f64) -> PResult<f64> {
    let start = p.pos;
    match p.number() {
        Some(n) if (low..=high).contains(&n) => Ok(n),
        Some(n) => Err(SyntaxError::new(
            p.span_from(start),
            format!("{what} must be from {low} to {high}, got {n}"),
        )),
        None => Err(p.unexpected(&[what])),
    }
}

pub(crate) fn twostep(p: &mut Parser) -> PResult<Command> {
    let mut command = Twostep {
        categorical: Vec::new(),
        continuous: Vec::new(),
        threshold: 0.0,
        branch: 8,
        levels: 3,
        euclidean: false,
        noise: 0.0,
        infile: None,
        memory: 64,
        include: false,
        unstandardized: Vec::new(),
        count: ClusterCount::Auto {
            max: 15,
            aic: false,
        },
        model_file: None,
        state_file: None,
        print_ic: false,
        print_count: false,
        print_summary: false,
        view: true,
        evaluation: Vec::new(),
        save: None,
    };
    while p.eat(Punct::Slash) {
        if at_end(p) {
            continue;
        }
        let categorical = p.at_keyword("CATEGORICAL");
        if p.eat_keyword("CATEGORICAL") || p.eat_keyword("CONTINUOUS") {
            p.eat_keyword("VARIABLES");
            p.eat(Punct::Eq);
            if !at_end(p) {
                let list = p.var_list()?;
                match categorical {
                    true => command.categorical = list,
                    false => command.continuous = list,
                }
            }
        } else if p.eat_keyword("CRITERIA") {
            p.eat(Punct::Eq);
            if !at_end(p) {
                command.threshold = 0.0;
                command.branch = 8;
                command.levels = 3;
            }
            while !at_end(p) {
                if p.eat_keyword("INITHRESHOLD") || p.eat_keyword("INITTHRESHOLD") {
                    p.expect(Punct::LParen)?;
                    command.threshold = number_from(p, "the initial threshold", 0.0, f64::MAX)?;
                    p.expect(Punct::RParen)?;
                } else if p.eat_keyword("MXBRANCH") {
                    command.branch = paren_whole(p, "MXBRANCH", 2)?;
                } else if p.eat_keyword("MXLEVEL") {
                    command.levels = paren_whole(p, "MXLEVEL", 1)?;
                } else {
                    return Err(p.unexpected(&["INITHRESHOLD", "MXBRANCH", "MXLEVEL"]));
                }
                p.eat(Punct::Comma);
            }
        } else if p.eat_keyword("DISTANCE") {
            p.eat(Punct::Eq);
            if p.eat_keyword("EUCLIDEAN") {
                command.euclidean = true;
            } else if p.eat_keyword("LIKELIHOOD") {
                command.euclidean = false;
            } else if !at_end(p) {
                return Err(p.unexpected(&["EUCLIDEAN", "LIKELIHOOD"]));
            }
        } else if p.eat_keyword("HANDLENOISE") {
            p.eat(Punct::Eq);
            if !at_end(p) {
                command.noise = number_from(p, "HANDLENOISE", 0.0, 100.0)?;
            }
        } else if p.eat_keyword("INFILE") {
            p.eat(Punct::Eq);
            if !at_end(p) {
                if !p.eat_keyword("FILE") {
                    return Err(p.unexpected(&["FILE"]));
                }
                p.eat(Punct::Eq);
                command.infile = Some(p.file("a file")?);
            }
        } else if p.eat_keyword("MEMALLOCATE") {
            p.eat(Punct::Eq);
            if !at_end(p) {
                command.memory = whole(p, "MEMALLOCATE", 4)?;
            }
        } else if p.eat_keyword("MISSING") {
            p.eat(Punct::Eq);
            if p.eat_keyword("EXCLUDE") {
                command.include = false;
            } else if p.eat_keyword("INCLUDE") {
                command.include = true;
            } else if !at_end(p) {
                return Err(p.unexpected(&["EXCLUDE", "INCLUDE"]));
            }
        } else if p.eat_keyword("NOSTANDARDIZE") {
            p.eat(Punct::Eq);
            if !at_end(p) {
                if !p.eat_keyword("VARIABLES") {
                    return Err(p.unexpected(&["VARIABLES"]));
                }
                p.eat(Punct::Eq);
                if !at_end(p) {
                    command.unstandardized = p.var_list()?;
                }
            }
        } else if p.eat_keyword("NUMCLUSTERS") {
            p.eat(Punct::Eq);
            if p.eat_keyword("AUTO") {
                let max = match p.peek() {
                    Some(TokenKind::Number(_)) => whole(p, "the largest number of clusters", 1)?,
                    _ => 15,
                };
                let aic = if p.eat_keyword("AIC") {
                    true
                } else {
                    p.eat_keyword("BIC");
                    false
                };
                command.count = ClusterCount::Auto { max, aic };
            } else if p.eat_keyword("FIXED") {
                p.eat(Punct::Eq);
                command.count = ClusterCount::Fixed(whole(p, "the number of clusters", 1)?);
            } else if !at_end(p) {
                return Err(p.unexpected(&["AUTO", "FIXED"]));
            }
        } else if p.eat_keyword("OUTFILE") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                let model = if p.eat_keyword("MODEL") {
                    true
                } else if p.eat_keyword("STATE") {
                    false
                } else {
                    return Err(p.unexpected(&["MODEL", "STATE"]));
                };
                p.eat(Punct::Eq);
                let file = Some(p.file("a file")?);
                match model {
                    true => command.model_file = file,
                    false => command.state_file = file,
                }
            }
        } else if p.eat_keyword("PRINT") {
            p.eat(Punct::Eq);
            if !at_end(p) {
                command.print_ic = false;
                command.print_count = false;
                command.print_summary = false;
            }
            while !at_end(p) {
                if p.eat_word("IC") {
                    command.print_ic = true;
                } else if p.eat_keyword("COUNT") {
                    command.print_count = true;
                } else if p.eat_keyword("SUMMARY") {
                    command.print_summary = true;
                } else {
                    return Err(p.unexpected(&["IC", "COUNT", "SUMMARY"]));
                }
                p.eat(Punct::Comma);
            }
        } else if p.eat_keyword("VIEWMODEL") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                if p.eat_keyword("DISPLAY") {
                    p.eat(Punct::Eq);
                    command.view = if p.eat_keyword("YES") {
                        true
                    } else if p.eat_keyword("NO") {
                        false
                    } else {
                        return Err(p.unexpected(&["YES", "NO"]));
                    };
                } else if p.eat_keyword("EVALUATIONFIELDS") {
                    p.eat(Punct::Eq);
                    command.evaluation = names_before_keyword(p)?;
                } else {
                    return Err(p.unexpected(&["DISPLAY", "EVALUATIONFIELDS"]));
                }
            }
        } else if p.eat_keyword("SAVE") {
            p.eat(Punct::Eq);
            if !at_end(p) {
                if !p.eat_keyword("CLUSTER") {
                    return Err(p.unexpected(&["CLUSTER"]));
                }
                let name = if p.eat_keyword("VARIABLE") {
                    p.eat(Punct::Eq);
                    Some(p.name("a variable name")?)
                } else {
                    None
                };
                command.save = Some(name);
            }
        } else {
            return Err(p.subcommand_error("TWOSTEP CLUSTER", &[]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    if command.categorical.is_empty() && command.continuous.is_empty() {
        return Err(SyntaxError::new(
            p.span(),
            "TWOSTEP CLUSTER needs CATEGORICAL VARIABLES= or CONTINUOUS VARIABLES=",
        ));
    }
    if command.euclidean && !command.categorical.is_empty() {
        return Err(SyntaxError::new(
            p.span(),
            "DISTANCE EUCLIDEAN takes only continuous variables; drop CATEGORICAL or use LIKELIHOOD",
        ));
    }
    Ok(Command::Twostep(Box::new(command)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnomalySave {
    Anomaly,
    PeerId,
    PeerSize,
    PeerPctSize,
    ReasonVar,
    ReasonMeasure,
    ReasonValue,
    ReasonNorm,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Detectanomaly {
    pub categorical: Option<Vec<VarItem>>,
    pub scale: Option<Vec<VarItem>>,
    pub id: Option<String>,
    pub except: Vec<VarItem>,
    pub apply: bool,
    pub missing_proportion: bool,
    pub min_peers: usize,
    pub max_peers: usize,
    pub ml_weight: f64,
    pub reasons: usize,
    pub percent: f64,
    pub count: Option<usize>,
    /// None for ANOMALYCUTPOINT=NONE.
    pub cutpoint: Option<f64>,
    /// In the order written, each with its name or root name if given.
    pub save: Vec<(AnomalySave, Option<String>)>,
    pub outfile: Option<String>,
    pub print_cps: bool,
    pub print_list: bool,
    pub print_norms: bool,
    pub print_summary: bool,
    pub print_reasons: bool,
}

/// `KEYWORD=` written in full, once within its subcommand.
fn keyword(
    p: &mut Parser,
    seen: &mut Vec<&'static str>,
    names: &[&'static str],
) -> PResult<&'static str> {
    let start = p.pos;
    let Some(&name) = names.iter().find(|n| p.eat_word(n)) else {
        return Err(p.unexpected(names));
    };
    if seen.contains(&name) {
        return Err(SyntaxError::new(
            p.span_from(start),
            format!("{name} is given twice in this subcommand"),
        ));
    }
    seen.push(name);
    Ok(name)
}

fn yes_no(p: &mut Parser) -> PResult<bool> {
    p.expect(Punct::Eq)?;
    if p.eat_word("YES") {
        Ok(true)
    } else if p.eat_word("NO") {
        Ok(false)
    } else {
        Err(p.unexpected(&["YES", "NO"]))
    }
}

pub(crate) fn detectanomaly(p: &mut Parser) -> PResult<Command> {
    let mut command = Detectanomaly {
        categorical: None,
        scale: None,
        id: None,
        except: Vec::new(),
        apply: false,
        missing_proportion: false,
        min_peers: 1,
        max_peers: 15,
        ml_weight: 6.0,
        reasons: 1,
        percent: 5.0,
        count: None,
        cutpoint: Some(2.0),
        save: Vec::new(),
        outfile: None,
        print_cps: false,
        print_list: true,
        print_norms: false,
        print_summary: false,
        print_reasons: false,
    };
    let mut subcommands: Vec<&'static str> = Vec::new();
    while p.eat(Punct::Slash) {
        if p.peek().is_none() {
            break;
        }
        let start = p.pos;
        let names = [
            "VARIABLES",
            "HANDLEMISSING",
            "CRITERIA",
            "SAVE",
            "OUTFILE",
            "PRINT",
        ];
        let Some(&sub) = names.iter().find(|n| p.eat_word(n)) else {
            return Err(p.subcommand_error("DETECTANOMALY", &[]));
        };
        if subcommands.contains(&sub) {
            return Err(SyntaxError::new(
                p.span_from(start),
                format!("DETECTANOMALY takes /{sub} once"),
            ));
        }
        subcommands.push(sub);
        let mut seen = Vec::new();
        match sub {
            "VARIABLES" => {
                while !at_end(p) {
                    match keyword(p, &mut seen, &["CATEGORICAL", "SCALE", "ID", "EXCEPT"])? {
                        "CATEGORICAL" => {
                            p.expect(Punct::Eq)?;
                            command.categorical = Some(names_before_keyword(p)?);
                        }
                        "SCALE" => {
                            p.expect(Punct::Eq)?;
                            command.scale = Some(names_before_keyword(p)?);
                        }
                        "ID" => {
                            p.expect(Punct::Eq)?;
                            command.id = Some(p.name("a variable name")?);
                        }
                        _ => {
                            p.expect(Punct::Eq)?;
                            let at = p.pos;
                            command.except = names_before_keyword(p)?;
                            if command.except.contains(&VarItem::All) {
                                return Err(SyntaxError::new(
                                    p.span_from(at),
                                    "EXCEPT cannot take ALL",
                                ));
                            }
                        }
                    }
                }
            }
            "HANDLEMISSING" => {
                while !at_end(p) {
                    match keyword(p, &mut seen, &["APPLY", "CREATEMISPROPVAR"])? {
                        "APPLY" => command.apply = yes_no(p)?,
                        _ => command.missing_proportion = yes_no(p)?,
                    }
                }
            }
            "CRITERIA" => {
                let names = [
                    "MINNUMPEERS",
                    "MAXNUMPEERS",
                    "MLWEIGHT",
                    "NUMREASONS",
                    "PCTANOMALOUSCASES",
                    "NUMANOMALOUSCASES",
                    "ANOMALYCUTPOINT",
                ];
                while !at_end(p) {
                    let name = keyword(p, &mut seen, &names)?;
                    p.expect(Punct::Eq)?;
                    match name {
                        "MINNUMPEERS" => command.min_peers = whole(p, name, 1)?,
                        "MAXNUMPEERS" => command.max_peers = whole(p, name, 1)?,
                        "MLWEIGHT" => {
                            let at = p.pos;
                            let m = number_from(p, name, 0.0, f64::MAX)?;
                            if m <= 0.0 {
                                return Err(SyntaxError::new(
                                    p.span_from(at),
                                    "MLWEIGHT must be positive",
                                ));
                            }
                            command.ml_weight = m;
                        }
                        "NUMREASONS" => command.reasons = whole(p, name, 0)?,
                        "PCTANOMALOUSCASES" => command.percent = number_from(p, name, 0.0, 100.0)?,
                        "NUMANOMALOUSCASES" => command.count = Some(whole(p, name, 0)?),
                        _ => {
                            command.cutpoint = if p.eat_word("NONE") {
                                None
                            } else {
                                Some(number_from(p, name, 0.0, f64::MAX)?)
                            }
                        }
                    }
                }
                if command.min_peers > command.max_peers {
                    return Err(SyntaxError::new(
                        p.span_from(start),
                        format!(
                            "MINNUMPEERS ({}) is larger than MAXNUMPEERS ({})",
                            command.min_peers, command.max_peers
                        ),
                    ));
                }
            }
            "SAVE" => {
                let kinds = [
                    ("ANOMALY", AnomalySave::Anomaly),
                    ("PEERID", AnomalySave::PeerId),
                    ("PEERSIZE", AnomalySave::PeerSize),
                    ("PEERPCTSIZE", AnomalySave::PeerPctSize),
                    ("REASONVAR", AnomalySave::ReasonVar),
                    ("REASONMEASURE", AnomalySave::ReasonMeasure),
                    ("REASONVALUE", AnomalySave::ReasonValue),
                    ("REASONNORM", AnomalySave::ReasonNorm),
                ];
                let words: Vec<&'static str> = kinds.iter().map(|k| k.0).collect();
                while !at_end(p) {
                    let name = keyword(p, &mut seen, &words)?;
                    let kind = kinds.iter().find(|k| k.0 == name).expect("listed").1;
                    let given = if p.eat(Punct::LParen) {
                        let given = p.name("a variable name")?;
                        p.expect(Punct::RParen)?;
                        Some(given)
                    } else {
                        None
                    };
                    command.save.push((kind, given));
                    p.eat(Punct::Comma);
                }
            }
            "OUTFILE" => {
                while !at_end(p) {
                    keyword(p, &mut seen, &["MODEL"])?;
                    p.expect(Punct::Eq)?;
                    command.outfile = Some(p.file("a file")?);
                }
            }
            _ => {
                let names = [
                    "CPS",
                    "ANOMALYLIST",
                    "NORMS",
                    "ANOMALYSUMMARY",
                    "REASONSUMMARY",
                    "NONE",
                ];
                if !at_end(p) {
                    command.print_list = false;
                }
                while !at_end(p) {
                    match keyword(p, &mut seen, &names)? {
                        "CPS" => command.print_cps = true,
                        "ANOMALYLIST" => command.print_list = true,
                        "NORMS" => command.print_norms = true,
                        "ANOMALYSUMMARY" => command.print_summary = true,
                        "REASONSUMMARY" => command.print_reasons = true,
                        _ => {}
                    }
                    p.eat(Punct::Comma);
                }
            }
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    Ok(Command::Detectanomaly(Box::new(command)))
}
