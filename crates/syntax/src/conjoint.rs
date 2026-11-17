//! Parsers of the conjoint family: ORTHOPLAN (an orthogonal main-effects plan), PLANCARDS (the
//! profiles of a plan as a list or as cards) and CONJOINT (part-worth utilities from rankings or
//! scores of the profiles).
//!
//! Rules from the Command Syntax Reference, in our words:
//! - ORTHOPLAN: FACTORS lists names, each with an optional label, then the levels in
//!   parentheses (numbers, each with an optional label, or quoted texts); the levels go to every
//!   name since the last list. REPLACE, OUTFILE=file, MINIMUM=n, HOLDOUT=n, MIXHOLD=YES|NO.
//! - PLANCARDS: FACTORS=varlist, FORMAT=LIST|CARD|BOTH (ALL is BOTH), TITLE and FOOTER with one
//!   or more strings (repeated subcommands add lines), OUTFILE=file. PAGINATE is obsolete and
//!   ignored.
//! - CONJOINT: PLAN and DATA name a file, a dataset or * (the active dataset); one of SEQUENCE,
//!   RANK or SCORE with the response variables; SUBJECT=variable; FACTORS (repeatable) with a
//!   model DISCRETE or LINEAR (each with MORE or LESS), IDEAL or ANTIIDEAL and new values or
//!   labels; PRINT=ALL|ANALYSIS|SIMULATION|NONE with SUMMARYONLY; UTILITY=file;
//!   PLOT=SUMMARY|SUBJECT|ALL|NONE. Except FACTORS the last of a subcommand counts.

use crate::ast::{Command, VarItem};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::optimal_scaling::outfile_name;
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

/// One level of an ORTHOPLAN factor: a number, or a text that becomes a coded level.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanLevel {
    pub value: Option<f64>,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlanFactor {
    pub name: String,
    pub label: Option<String>,
    /// None: the levels come from the value labels of the active dataset.
    pub levels: Option<Vec<PlanLevel>>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Orthoplan {
    pub factors: Vec<PlanFactor>,
    pub replace: bool,
    pub outfile: Option<String>,
    pub minimum: Option<usize>,
    pub holdout: usize,
    pub mixhold: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CardFormat {
    #[default]
    List,
    Card,
    Both,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Plancards {
    pub factors: Option<Vec<VarItem>>,
    pub format: CardFormat,
    pub titles: Vec<String>,
    pub footers: Vec<String>,
    pub outfile: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseKind {
    Sequence,
    Rank,
    Score,
}

/// The expected direction of a DISCRETE or LINEAR factor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    More,
    Less,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConjointModel {
    Discrete(Option<Direction>),
    Linear(Option<Direction>),
    Ideal,
    Antiideal,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConjointFactor {
    pub name: String,
    pub label: Option<String>,
    pub model: ConjointModel,
    /// Per existing value from the smallest up: a new value and a new label, either optional.
    pub recode: Vec<(Option<f64>, Option<String>)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConjointPrint {
    #[default]
    All,
    Analysis,
    Simulation,
    None,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Conjoint {
    /// None: not given; Some(None): the active dataset; Some(Some(file)).
    pub plan: Option<Option<String>>,
    pub data: Option<Option<String>>,
    pub responses: (ResponseKind, Vec<VarItem>),
    pub subject: Option<String>,
    pub factors: Vec<ConjointFactor>,
    pub print: ConjointPrint,
    pub summary_only: bool,
    pub utility: Option<String>,
    pub plot_summary: bool,
    pub plot_subject: bool,
}

/// Steps over an optional slash before the next subcommand; false at the end of the command.
fn next_subcommand(p: &mut Parser, first: &mut bool) -> bool {
    let slash = p.eat(Punct::Slash);
    if !slash && !*first {
        return false;
    }
    *first = false;
    p.peek().is_some()
}

fn positive(p: &mut Parser, what: &str) -> PResult<usize> {
    p.eat(Punct::Eq);
    let start = p.pos;
    match p.number() {
        Some(n) if n.fract() == 0.0 && n >= 1.0 => Ok(n as usize),
        Some(n) => Err(SyntaxError::new(
            p.span_from(start),
            format!("{what} must be a positive whole number, got {n}"),
        )),
        None => Err(p.unexpected(&[what])),
    }
}

fn optional_string(p: &mut Parser) -> PResult<Option<String>> {
    match p.peek() {
        Some(TokenKind::Str(_)) => p.string("a label").map(Some),
        _ => Ok(None),
    }
}

pub(crate) fn orthoplan(p: &mut Parser) -> PResult<Command> {
    let mut c = Orthoplan::default();
    let mut first = true;
    while next_subcommand(p, &mut first) {
        if p.eat_keyword("FACTORS") {
            p.eat(Punct::Eq);
            // names wait for the level list that follows them
            let mut waiting = 0;
            while !at_end(p) {
                if p.eat(Punct::LParen) {
                    if waiting == 0 {
                        return Err(p.unexpected(&["a factor name"]));
                    }
                    let mut levels = Vec::new();
                    while !p.eat(Punct::RParen) {
                        if let Some(value) = p.number() {
                            let label = optional_string(p)?;
                            levels.push(PlanLevel {
                                value: Some(value),
                                label,
                            });
                        } else if let Some(TokenKind::Str(_)) = p.peek() {
                            let label = Some(p.string("a level")?);
                            levels.push(PlanLevel { value: None, label });
                        } else {
                            return Err(p.unexpected(&["a value", "a quoted level", "')'"]));
                        }
                        p.eat(Punct::Comma);
                    }
                    let n = c.factors.len();
                    for factor in &mut c.factors[n - waiting..] {
                        factor.levels = Some(levels.clone());
                    }
                    waiting = 0;
                } else {
                    let name = p.name("a factor name")?;
                    let label = optional_string(p)?;
                    c.factors.push(PlanFactor {
                        name,
                        label,
                        levels: None,
                    });
                    waiting += 1;
                }
                p.eat(Punct::Comma);
            }
        } else if p.eat_keyword("REPLACE") {
            c.replace = true;
        } else if p.eat_keyword("OUTFILE") {
            p.eat(Punct::Eq);
            c.outfile = Some(outfile_name(p)?);
        } else if p.eat_keyword("MINIMUM") {
            c.minimum = Some(positive(p, "the minimum number of cases")?);
        } else if p.eat_keyword("HOLDOUT") {
            c.holdout = positive(p, "the number of holdout cases")?;
        } else if p.eat_keyword("MIXHOLD") {
            p.eat(Punct::Eq);
            c.mixhold = if p.eat_keyword("YES") {
                true
            } else if p.eat_keyword("NO") {
                false
            } else {
                return Err(p.unexpected(&["YES", "NO"]));
            };
        } else {
            return Err(p.subcommand_error("ORTHOPLAN", &[]));
        }
    }
    p.finish(&["'/'"])?;
    Ok(Command::Orthoplan(Box::new(c)))
}

/// One or more strings, each a line.
fn lines(p: &mut Parser, into: &mut Vec<String>) -> PResult<()> {
    p.eat(Punct::Eq);
    if !matches!(p.peek(), Some(TokenKind::Str(_))) {
        return Err(p.unexpected(&["a quoted string"]));
    }
    while let Some(TokenKind::Str(_)) = p.peek() {
        into.push(p.string("a quoted string")?);
        p.eat(Punct::Comma);
    }
    Ok(())
}

pub(crate) fn plancards(p: &mut Parser) -> PResult<Command> {
    let mut c = Plancards::default();
    let mut first = true;
    while next_subcommand(p, &mut first) {
        if p.eat_keyword("FACTORS") {
            p.eat(Punct::Eq);
            c.factors = Some(p.var_list()?);
        } else if p.eat_keyword("FORMAT") {
            p.eat(Punct::Eq);
            c.format = if p.eat_keyword("LIST") {
                CardFormat::List
            } else if p.eat_keyword("CARD") {
                CardFormat::Card
            } else if p.eat_keyword("BOTH") || p.eat_word("ALL") {
                CardFormat::Both
            } else {
                return Err(p.unexpected(&["LIST", "CARD", "BOTH"]));
            };
        } else if p.eat_keyword("TITLE") {
            lines(p, &mut c.titles)?;
        } else if p.eat_keyword("FOOTER") {
            lines(p, &mut c.footers)?;
        } else if p.eat_keyword("OUTFILE") {
            p.eat(Punct::Eq);
            c.outfile = Some(outfile_name(p)?);
        } else if p.eat_keyword("PAGINATE") {
            // obsolete: pages are the viewer's business
            while !at_end(p) {
                p.pos += 1;
            }
        } else {
            return Err(p.subcommand_error("PLANCARDS", &[]));
        }
    }
    p.finish(&["'/'"])?;
    Ok(Command::Plancards(Box::new(c)))
}

/// `*` (the active dataset) or a file or dataset name.
fn source(p: &mut Parser) -> PResult<Option<String>> {
    p.eat(Punct::Eq);
    if p.eat(Punct::Star) {
        return Ok(None);
    }
    p.file("a file or dataset name, or *").map(Some)
}

fn direction(p: &mut Parser) -> Option<Direction> {
    if p.eat_keyword("MORE") {
        Some(Direction::More)
    } else if p.eat_keyword("LESS") {
        Some(Direction::Less)
    } else {
        None
    }
}

fn conjoint_factors(p: &mut Parser, into: &mut Vec<ConjointFactor>) -> PResult<()> {
    p.eat(Punct::Eq);
    let mut waiting = 0;
    let mut any = false;
    while !at_end(p) {
        if p.eat(Punct::LParen) {
            if waiting == 0 {
                return Err(p.unexpected(&["a factor name"]));
            }
            let model = if p.eat_keyword("DISCRETE") {
                ConjointModel::Discrete(direction(p))
            } else if p.eat_keyword("LINEAR") {
                ConjointModel::Linear(direction(p))
            } else if p.eat_keyword("IDEAL") {
                ConjointModel::Ideal
            } else if p.eat_keyword("ANTIIDEAL") {
                ConjointModel::Antiideal
            } else {
                ConjointModel::Discrete(None)
            };
            let mut recode = Vec::new();
            while !p.eat(Punct::RParen) {
                if let Some(value) = p.number() {
                    recode.push((Some(value), optional_string(p)?));
                } else if let Some(TokenKind::Str(_)) = p.peek() {
                    recode.push((None, Some(p.string("a label")?)));
                } else {
                    return Err(p.unexpected(&["a value", "a quoted label", "')'"]));
                }
                p.eat(Punct::Comma);
            }
            let n = into.len();
            for factor in &mut into[n - waiting..] {
                factor.model = model;
                factor.recode = recode.clone();
            }
            waiting = 0;
        } else {
            let name = p.name("a factor name")?;
            let label = optional_string(p)?;
            into.push(ConjointFactor {
                name,
                label,
                model: ConjointModel::Discrete(None),
                recode: Vec::new(),
            });
            waiting += 1;
            any = true;
        }
        p.eat(Punct::Comma);
    }
    if !any {
        return Err(p.unexpected(&["a factor name"]));
    }
    Ok(())
}

pub(crate) fn conjoint(p: &mut Parser) -> PResult<Command> {
    let start = p.pos;
    let mut c = Conjoint {
        plan: None,
        data: None,
        responses: (ResponseKind::Rank, Vec::new()),
        subject: None,
        factors: Vec::new(),
        print: ConjointPrint::All,
        summary_only: false,
        utility: None,
        plot_summary: false,
        plot_subject: false,
    };
    let mut kind: Option<ResponseKind> = None;
    let mut first = true;
    while next_subcommand(p, &mut first) {
        let at = p.pos;
        let response = if p.eat_keyword("SEQUENCE") {
            Some(ResponseKind::Sequence)
        } else if p.eat_keyword("RANK") {
            Some(ResponseKind::Rank)
        } else if p.eat_keyword("SCORE") {
            Some(ResponseKind::Score)
        } else {
            None
        };
        if let Some(response) = response {
            if kind.is_some_and(|k| k != response) {
                return Err(SyntaxError::new(
                    p.span_from(at),
                    "give only one of SEQUENCE, RANK and SCORE",
                ));
            }
            kind = Some(response);
            p.eat(Punct::Eq);
            c.responses = (response, p.var_list()?);
        } else if p.eat_keyword("PLAN") {
            c.plan = Some(source(p)?);
        } else if p.eat_keyword("DATA") {
            c.data = Some(source(p)?);
        } else if p.eat_keyword("SUBJECT") {
            p.eat(Punct::Eq);
            c.subject = Some(p.name("a variable name")?);
        } else if p.eat_keyword("FACTORS") {
            conjoint_factors(p, &mut c.factors)?;
        } else if p.eat_keyword("PRINT") {
            p.eat(Punct::Eq);
            (c.print, c.summary_only) = (ConjointPrint::All, false);
            while !at_end(p) {
                if p.eat_word("ALL") {
                    c.print = ConjointPrint::All;
                } else if p.eat_keyword("ANALYSIS") {
                    c.print = ConjointPrint::Analysis;
                } else if p.eat_keyword("SIMULATION") {
                    c.print = ConjointPrint::Simulation;
                } else if p.eat_keyword("NONE") {
                    c.print = ConjointPrint::None;
                } else if p.eat_keyword("SUMMARYONLY") {
                    c.summary_only = true;
                } else {
                    return Err(p.unexpected(&[
                        "ALL",
                        "ANALYSIS",
                        "SIMULATION",
                        "NONE",
                        "SUMMARYONLY",
                    ]));
                }
            }
        } else if p.eat_keyword("UTILITY") {
            p.eat(Punct::Eq);
            c.utility = Some(outfile_name(p)?);
        } else if p.eat_keyword("PLOT") {
            p.eat(Punct::Eq);
            // the subcommand alone asks for the summary charts
            (c.plot_summary, c.plot_subject) = (at_end(p), false);
            while !at_end(p) {
                if p.eat_keyword("SUMMARY") {
                    c.plot_summary = true;
                } else if p.eat_keyword("SUBJECT") {
                    c.plot_subject = true;
                } else if p.eat_word("ALL") {
                    (c.plot_summary, c.plot_subject) = (true, true);
                } else if p.eat_keyword("NONE") {
                    (c.plot_summary, c.plot_subject) = (false, false);
                } else {
                    return Err(p.unexpected(&["SUMMARY", "SUBJECT", "ALL", "NONE"]));
                }
            }
        } else {
            return Err(p.subcommand_error("CONJOINT", &[]));
        }
    }
    p.finish(&["'/'"])?;
    if kind.is_none() {
        return Err(SyntaxError::new(
            p.span_from(start),
            "CONJOINT needs SEQUENCE, RANK or SCORE with the response variables",
        ));
    }
    let active = |s: &Option<Option<String>>| !matches!(s, Some(Some(_)));
    if active(&c.plan) && active(&c.data) {
        return Err(SyntaxError::new(
            p.span_from(start),
            "CONJOINT needs a file or dataset on PLAN or DATA: the active dataset cannot be both",
        ));
    }
    Ok(Command::Conjoint(Box::new(c)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    fn one(source: &str) -> Command {
        let (mut parsed, errors) = parse(source);
        assert!(errors.is_empty(), "{source}: {errors:?}");
        parsed.remove(0).command
    }

    fn err(source: &str) -> String {
        let (_, mut errors) = parse(source);
        assert_eq!(errors.len(), 1, "{source}: {errors:?}");
        errors.remove(0).message
    }

    #[test]
    fn orthoplan_factors_share_a_level_list() {
        let Command::Orthoplan(o) = one(
            "ORTHOPLAN FACTORS=size 'Pack size' (1 'small' 2 'large') fruit taste 'Taste' \
             ('sweet' 'sour' 'bitter') /MIN=6 /HOLD 3 /MIXH YES /OUT='plan.sav'.",
        ) else {
            panic!()
        };
        assert_eq!(o.factors.len(), 3);
        assert_eq!(o.factors[0].label.as_deref(), Some("Pack size"));
        assert_eq!(o.factors[0].levels.as_ref().unwrap()[1].value, Some(2.0));
        assert_eq!(o.factors[1].levels, o.factors[2].levels);
        assert_eq!(o.factors[1].levels.as_ref().unwrap()[2].value, None);
        assert_eq!((o.minimum, o.holdout, o.mixhold), (Some(6), 3, true));
        assert_eq!(o.outfile.as_deref(), Some("plan.sav"));
        let Command::Orthoplan(o) = one("ORTHOPLAN /REPLACE.") else {
            panic!()
        };
        assert!(o.replace && o.factors.is_empty());
        assert!(err("ORTHOPLAN /MINIMUM=2.5.").contains("positive whole"));
    }

    #[test]
    fn plancards_lines_add_up() {
        let Command::Plancards(c) = one(
            "PLANCARDS FORMAT=ALL /TITLE='Card )CARD' ' ' /TITLE 'more' /FOOTER='Rank:' \
             /FACTORS=a b /OUTFILE='cards.txt'.",
        ) else {
            panic!()
        };
        assert_eq!(c.format, CardFormat::Both);
        assert_eq!(c.titles, ["Card )CARD", " ", "more"]);
        assert_eq!(c.footers, ["Rank:"]);
        assert_eq!(c.factors.unwrap().len(), 2);
    }

    #[test]
    fn conjoint_models_and_options() {
        let Command::Conjoint(c) = one(
            "CONJOINT PLAN='plan.sav' /FACTORS=price (LINEAR LESS) speed (IDEAL 70 100 130) \
             seats (DISC MORE '2 seats') colour /SUBJECT=id /RANK=r1 TO r9 \
             /PRINT=ANALYSIS SUMMARYONLY /PLOT /UTIL='u.sav'.",
        ) else {
            panic!()
        };
        assert_eq!(c.plan, Some(Some("plan.sav".into())));
        assert_eq!(c.data, None);
        assert_eq!(c.responses.0, ResponseKind::Rank);
        assert_eq!(
            c.factors[0].model,
            ConjointModel::Linear(Some(Direction::Less))
        );
        assert_eq!(c.factors[1].model, ConjointModel::Ideal);
        assert_eq!(c.factors[1].recode.len(), 3);
        assert_eq!(c.factors[2].recode, [(None, Some("2 seats".to_string()))]);
        assert_eq!(c.factors[3].model, ConjointModel::Discrete(None));
        assert_eq!((c.print, c.summary_only), (ConjointPrint::Analysis, true));
        assert!(c.plot_summary && !c.plot_subject);
        assert!(err("CONJOINT PLAN=* /SCORE=a b.").contains("cannot be both"));
        assert!(err("CONJOINT PLAN='p.sav' /RANK=a /SCORE=b.").contains("only one"));
        assert!(err("CONJOINT PLAN='p.sav'.").contains("SEQUENCE, RANK or SCORE"));
    }
}
