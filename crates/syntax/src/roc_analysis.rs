//! Parser of ROC ANALYSIS: ROC curves of several test variables with paired or
//! independent-group comparisons of their areas, precision-recall curves and classifier
//! metrics.
//!
//! Rules from the Command Syntax Reference (ROC ANALYSIS):
//! - `varlist BY state(value)` as for ROC: numeric test variables, a state variable of any
//!   type and the value that marks the positive state.
//! - Every keyword takes `KEY = value` or `KEY(value)`. MISSING: USERMISSING = EXCLUDE |
//!   INCLUDE. CRITERIA: CUTOFF = INCLUDE | EXCLUDE, TESTPOS = LARGE | SMALL, CI = n with
//!   0 < n < 100, DISTRIBUTION = FREE | NEGEXPO. DESIGN: PAIR = TRUE | FALSE, GROUP = var,
//!   SELECT = LEVEL(v1 v2) | MIDPOINT | CUTPOINT(c). PLOT: CURVE = ROC[(REFERENCE)]
//!   [PR[(REFERENCE)]] | NONE, PR_INTERPOLATE (also PRINTERPOLATE) = TP | FP, MODELQUALITY =
//!   TRUE | FALSE. PRINT: SE = TRUE | FALSE, COORDINATES = [ROC] [PR], CLASSIFIER = TRUE |
//!   FALSE.
//! - Subcommands in any order. A repeated subcommand: only the last counts, with a warning.
//!   A keyword given again with another value: the later one counts, with a warning; given
//!   again with the same value it is silently accepted.

use crate::ast::{Command, Value, VarItem};
use crate::error::SyntaxError;
use crate::glm::list_until;
use crate::lexer::Punct;
use crate::parser::{PResult, Parser};
use crate::pplot::choice;
use crate::procedures::at_end;

#[derive(Debug, Clone, PartialEq)]
pub struct RocAnalysis {
    pub tests: Vec<VarItem>,
    pub state: String,
    /// The value of the state variable that marks the positive state.
    pub positive: Value,
    /// MISSING USERMISSING=INCLUDE.
    pub include: bool,
    pub criteria: RocCriteria,
    pub design: RocDesign,
    pub plot: RocPlot,
    pub print: RocPrint,
    /// Syntax warnings: repeated subcommands and keywords whose later value replaced an
    /// earlier one.
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RocCriteria {
    /// CUTOFF=EXCLUDE: a value equal to the cutoff tests negative.
    pub exclude_cutoff: bool,
    /// TESTPOS=SMALL: smaller test values point to the positive state.
    pub small_positive: bool,
    /// The confidence level in percent.
    pub ci: f64,
    /// DISTRIBUTION=NEGEXPO: the bi-negative exponential standard error.
    pub negexpo: bool,
}

impl Default for RocCriteria {
    fn default() -> Self {
        RocCriteria {
            exclude_cutoff: false,
            small_positive: false,
            ci: 95.0,
            negexpo: false,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RocDesign {
    pub pair: bool,
    pub group: Option<String>,
    pub select: Option<RocSelect>,
}

/// How SELECT splits the cases by the GROUP variable.
#[derive(Debug, Clone, PartialEq)]
pub enum RocSelect {
    /// LEVEL(v1 v2): the cases with the first value against those with the second.
    Levels(Value, Value),
    /// At or above the midpoint of the smallest and largest group value, against below it.
    Midpoint,
    /// At or above the value, against below it.
    Cutpoint(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RocPlot {
    pub roc: bool,
    pub reference: bool,
    pub pr: bool,
    pub pr_reference: bool,
    /// PR_INTERPOLATE=FP: the precision-recall curve is filled in along the false positives.
    pub interpolate_fp: bool,
    pub model_quality: bool,
}

impl Default for RocPlot {
    fn default() -> Self {
        RocPlot {
            roc: true,
            reference: false,
            pr: false,
            pr_reference: false,
            interpolate_fp: false,
            model_quality: false,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RocPrint {
    pub se: bool,
    pub coordinates_roc: bool,
    pub coordinates_pr: bool,
    pub classifier: bool,
}

const SUBCOMMANDS: [&str; 5] = ["MISSING", "CRITERIA", "DESIGN", "PLOT", "PRINT"];
const BOOL: [(&str, bool); 2] = [("TRUE", true), ("FALSE", false)];

/// `= KEYWORD` or `(KEYWORD)` from `choices`.
fn setting<T: Copy>(p: &mut Parser, choices: &[(&'static str, T)]) -> PResult<T> {
    if p.eat(Punct::LParen) {
        let value = choice(p, choices)?;
        p.expect(Punct::RParen)?;
        return Ok(value);
    }
    choice(p, choices)
}

/// The keywords seen in the current subcommand, with their values as text.
struct Seen<'a> {
    keys: Vec<(&'static str, String)>,
    warnings: &'a mut Vec<String>,
}

impl Seen<'_> {
    fn note(&mut self, key: &'static str, value: impl std::fmt::Debug) {
        let value = format!("{value:?}");
        match self.keys.iter_mut().find(|(k, _)| *k == key) {
            Some((_, old)) if *old != value => {
                self.warnings
                    .push(format!("{key} is given twice; the later value counts"));
                *old = value;
            }
            Some(_) => {}
            None => self.keys.push((key, value)),
        }
    }
}

/// ROC[(REFERENCE)], PR[(REFERENCE)] and NONE, until the next keyword of PLOT.
fn curves(p: &mut Parser, plot: &mut RocPlot) -> PResult<()> {
    let parens = !p.eat(Punct::Eq) && p.eat(Punct::LParen);
    let (mut roc, mut pr, mut none) = (None, None, false);
    loop {
        let item = if p.eat_word("ROC") {
            &mut roc
        } else if p.eat_word("PR") {
            &mut pr
        } else if p.eat_keyword("NONE") {
            (roc, pr, none) = (None, None, true);
            p.eat(Punct::Comma);
            continue;
        } else {
            break;
        };
        none = false;
        let mut reference = false;
        if p.eat(Punct::LParen) {
            if !p.eat_keyword("REFERENCE") {
                return Err(p.unexpected(&["REFERENCE"]));
            }
            p.expect(Punct::RParen)?;
            reference = true;
        }
        *item = Some(reference);
        p.eat(Punct::Comma);
    }
    if parens {
        p.expect(Punct::RParen)?;
    }
    if roc.is_none() && pr.is_none() && !none {
        return Err(p.unexpected(&["ROC", "PR", "NONE"]));
    }
    // of NONE and the curves, what comes last counts
    plot.roc = roc.is_some();
    plot.pr = pr.is_some();
    plot.reference = roc == Some(true);
    plot.pr_reference = pr == Some(true);
    Ok(())
}

pub(crate) fn roc_analysis(p: &mut Parser) -> PResult<Command> {
    let tests = list_until(p, &["BY"])?;
    if !p.eat_word("BY") {
        return Err(p.unexpected(&["BY"]));
    }
    let state = p.name("the state variable")?;
    p.expect(Punct::LParen)?;
    let Some(positive) = p.value() else {
        return Err(p.unexpected(&["the value of the positive state"]));
    };
    p.expect(Punct::RParen)?;
    let mut command = RocAnalysis {
        tests,
        state,
        positive,
        include: false,
        criteria: RocCriteria::default(),
        design: RocDesign::default(),
        plot: RocPlot::default(),
        print: RocPrint::default(),
        warnings: Vec::new(),
    };
    let mut given: Vec<&str> = Vec::new();
    while p.eat(Punct::Slash) {
        let Some(&name) = SUBCOMMANDS.iter().find(|s| p.at_keyword(s)) else {
            return Err(p.subcommand_error("ROC ANALYSIS", &SUBCOMMANDS));
        };
        p.pos += 1;
        p.eat(Punct::Eq);
        if given.contains(&name) {
            command
                .warnings
                .push(format!("{name} is given twice; only the last one counts"));
        }
        given.push(name);
        let mut warnings = std::mem::take(&mut command.warnings);
        let mut seen = Seen {
            keys: Vec::new(),
            warnings: &mut warnings,
        };
        match name {
            "MISSING" => {
                command.include = false;
                while !at_end(p) {
                    // USERMISSING may be left out
                    p.eat_keyword("USERMISSING");
                    command.include = setting(p, &[("EXCLUDE", false), ("INCLUDE", true)])?;
                    seen.note("USERMISSING", command.include);
                    p.eat(Punct::Comma);
                }
            }
            "CRITERIA" => {
                let c = &mut command.criteria;
                *c = RocCriteria::default();
                while !at_end(p) {
                    if p.eat_keyword("CUTOFF") {
                        c.exclude_cutoff = setting(p, &[("INCLUDE", false), ("EXCLUDE", true)])?;
                        seen.note("CUTOFF", c.exclude_cutoff);
                    } else if p.eat_keyword("TESTPOS") {
                        c.small_positive = setting(p, &[("LARGE", false), ("SMALL", true)])?;
                        seen.note("TESTPOS", c.small_positive);
                    } else if p.eat_keyword("DISTRIBUTION") {
                        c.negexpo = setting(p, &[("FREE", false), ("NEGEXPO", true)])?;
                        seen.note("DISTRIBUTION", c.negexpo);
                    } else if p.eat_word("CI") {
                        let parens = !p.eat(Punct::Eq) && p.eat(Punct::LParen);
                        let start = p.pos;
                        let Some(level) = p.number() else {
                            return Err(p.unexpected(&["a confidence level"]));
                        };
                        if !(level > 0.0 && level < 100.0) {
                            return Err(SyntaxError::new(
                                p.span_from(start),
                                format!("CI takes a level between 0 and 100, got {level}"),
                            ));
                        }
                        if parens {
                            p.expect(Punct::RParen)?;
                        }
                        c.ci = level;
                        seen.note("CI", level);
                    } else {
                        return Err(p.unexpected(&["CUTOFF", "TESTPOS", "CI", "DISTRIBUTION"]));
                    }
                    p.eat(Punct::Comma);
                }
            }
            "DESIGN" => {
                let d = &mut command.design;
                *d = RocDesign::default();
                while !at_end(p) {
                    if p.eat_keyword("PAIR") {
                        d.pair = setting(p, &BOOL)?;
                        seen.note("PAIR", d.pair);
                    } else if p.eat_keyword("GROUP") {
                        let parens = !p.eat(Punct::Eq) && p.eat(Punct::LParen);
                        let group = p.name("the grouping variable")?;
                        if parens {
                            p.expect(Punct::RParen)?;
                        }
                        seen.note("GROUP", group.to_uppercase());
                        d.group = Some(group);
                    } else if p.eat_keyword("SELECT") {
                        let parens = !p.eat(Punct::Eq) && p.eat(Punct::LParen);
                        let select = if p.eat_keyword("LEVEL") {
                            p.expect(Punct::LParen)?;
                            let mut values = Vec::new();
                            while let Some(v) = p.value() {
                                values.push(v);
                                p.eat(Punct::Comma);
                            }
                            p.expect(Punct::RParen)?;
                            let [first, second]: [Value; 2] = values.try_into().map_err(|_| {
                                SyntaxError::new(p.span(), "LEVEL takes two group values")
                            })?;
                            RocSelect::Levels(first, second)
                        } else if p.eat_keyword("MIDPOINT") {
                            RocSelect::Midpoint
                        } else if p.eat_keyword("CUTPOINT") {
                            p.expect(Punct::LParen)?;
                            let Some(cut) = p.number() else {
                                return Err(p.unexpected(&["a cut point"]));
                            };
                            p.expect(Punct::RParen)?;
                            RocSelect::Cutpoint(cut)
                        } else {
                            return Err(p.unexpected(&["LEVEL", "MIDPOINT", "CUTPOINT"]));
                        };
                        if parens {
                            p.expect(Punct::RParen)?;
                        }
                        seen.note("SELECT", &select);
                        d.select = Some(select);
                    } else {
                        return Err(p.unexpected(&["PAIR", "GROUP", "SELECT"]));
                    }
                    p.eat(Punct::Comma);
                }
            }
            "PLOT" => {
                let plot = &mut command.plot;
                *plot = RocPlot::default();
                while !at_end(p) {
                    if p.eat_keyword("CURVE") {
                        curves(p, plot)?;
                        seen.note(
                            "CURVE",
                            (plot.roc, plot.reference, plot.pr, plot.pr_reference),
                        );
                    } else if p.eat_keyword("PR_INTERPOLATE") || p.eat_keyword("PRINTERPOLATE") {
                        plot.interpolate_fp = setting(p, &[("TP", false), ("FP", true)])?;
                        seen.note("PR_INTERPOLATE", plot.interpolate_fp);
                    } else if p.eat_keyword("MODELQUALITY") {
                        plot.model_quality = setting(p, &BOOL)?;
                        seen.note("MODELQUALITY", plot.model_quality);
                    } else {
                        return Err(p.unexpected(&["CURVE", "PR_INTERPOLATE", "MODELQUALITY"]));
                    }
                    p.eat(Punct::Comma);
                }
            }
            _ => {
                let print = &mut command.print;
                *print = RocPrint::default();
                while !at_end(p) {
                    if p.eat_word("SE") {
                        print.se = setting(p, &BOOL)?;
                        seen.note("SE", print.se);
                    } else if p.eat_keyword("CLASSIFIER") {
                        print.classifier = setting(p, &BOOL)?;
                        seen.note("CLASSIFIER", print.classifier);
                    } else if p.eat_keyword("COORDINATES") {
                        let parens = !p.eat(Punct::Eq) && p.eat(Punct::LParen);
                        let (mut roc, mut pr) = (false, false);
                        loop {
                            if p.eat_word("ROC") {
                                roc = true;
                            } else if p.eat_word("PR") {
                                pr = true;
                            } else {
                                break;
                            }
                            p.eat(Punct::Comma);
                        }
                        if parens {
                            p.expect(Punct::RParen)?;
                        }
                        if !roc && !pr {
                            return Err(p.unexpected(&["ROC", "PR"]));
                        }
                        print.coordinates_roc |= roc;
                        print.coordinates_pr |= pr;
                    } else {
                        return Err(p.unexpected(&["SE", "COORDINATES", "CLASSIFIER"]));
                    }
                    p.eat(Punct::Comma);
                }
            }
        }
        command.warnings = warnings;
    }
    p.finish(&["'/'"])?;
    Ok(Command::RocAnalysis(Box::new(command)))
}
