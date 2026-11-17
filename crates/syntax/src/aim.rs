//! Parsers of AIM (attribute importance) and SELECTPRED (predictor screening and ranking).
//!
//! AIM, rules from the Command Syntax Reference in our words:
//! - The grouping variable comes first; CATEGORICAL and CONTINUOUS name the variables, at
//!   least one of them is needed. Subcommands come in any order; a repeated one replaces the
//!   earlier. Keywords may be shortened to three letters.
//! - CRITERIA ADJUST=BONFERRONI|NONE CI=95 HIDENOTSIG=NO|YES SHOWREFLINE=YES|NO; MISSING
//!   EXCLUDE|INCLUDE; PLOT CATEGORY, CLUSTER [(TYPE=BAR|PIE)], ERRORBAR,
//!   IMPORTANCE [([X=GROUP|VARIABLE] [Y=TEST|PVALUE])].
//!
//! SELECTPRED: the dependent variable may carry (MLEVEL=S|O|N) and be followed by BY factors
//! and WITH covariates. Every subcommand is optional and may be given once; command,
//! subcommand and keyword names are written in full, the equals signs and parentheses are
//! required, a keyword may appear once within its subcommand and an empty subcommand is
//! ignored. EXCEPT VARIABLES=list; SCREENING STATUS PCTMISSING PCTEQUAL PCTUNEQUAL STDDEV CV;
//! CRITERIA SIZE PCUTOFF RANKING TIMER SHOWUNSELECTED; MISSING USERMISSING; PRINT CPS EXCLUDED
//! SELECTED NONE SUMMARY[(criteria)]; PLOT SUMMARY.

use crate::ast::{Command, VarItem};
use crate::error::SyntaxError;
use crate::glm::list_until;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

/// PLOT IMPORTANCE: one chart per variable (X=GROUP) or per group (X=VARIABLE), showing the
/// test statistics or -log10 of the significance (Y=PVALUE).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AimImportance {
    pub by_variable: bool,
    pub pvalue: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Aim {
    pub group: String,
    pub categorical: Vec<VarItem>,
    pub continuous: Vec<VarItem>,
    pub bonferroni: bool,
    /// The confidence level in percent.
    pub ci: f64,
    pub hide_not_significant: bool,
    pub reference_lines: bool,
    pub include: bool,
    pub category_plot: bool,
    /// PLOT CLUSTER: Some(true) for a pie, Some(false) for bars.
    pub cluster_plot: Option<bool>,
    pub error_bar: bool,
    pub importance: Option<AimImportance>,
}

fn yes_no(p: &mut Parser) -> PResult<bool> {
    p.eat(Punct::Eq);
    if p.eat_keyword("YES") {
        Ok(true)
    } else if p.eat_keyword("NO") {
        Ok(false)
    } else {
        Err(p.unexpected(&["YES", "NO"]))
    }
}

pub(crate) fn aim(p: &mut Parser) -> PResult<Command> {
    let group = p.name("the grouping variable")?;
    let mut command = Aim {
        group,
        categorical: Vec::new(),
        continuous: Vec::new(),
        bonferroni: true,
        ci: 95.0,
        hide_not_significant: false,
        reference_lines: true,
        include: false,
        category_plot: false,
        cluster_plot: None,
        error_bar: false,
        importance: None,
    };
    while p.eat(Punct::Slash) {
        if at_end(p) {
            continue;
        }
        let categorical = p.at_keyword("CATEGORICAL");
        if p.eat_keyword("CATEGORICAL") || p.eat_keyword("CONTINUOUS") {
            p.eat(Punct::Eq);
            let list = p.var_list()?;
            match categorical {
                true => command.categorical = list,
                false => command.continuous = list,
            }
        } else if p.eat_keyword("CRITERIA") {
            p.eat(Punct::Eq);
            (command.bonferroni, command.ci) = (true, 95.0);
            (command.hide_not_significant, command.reference_lines) = (false, true);
            while !at_end(p) {
                if p.eat_keyword("ADJUST") {
                    p.eat(Punct::Eq);
                    command.bonferroni = if p.eat_keyword("BONFERRONI") {
                        true
                    } else if p.eat_keyword("NONE") {
                        false
                    } else {
                        return Err(p.unexpected(&["BONFERRONI", "NONE"]));
                    };
                } else if p.eat_word("CI") {
                    p.eat(Punct::Eq);
                    let start = p.pos;
                    command.ci = match p.number() {
                        Some(n) if n > 0.0 && n < 100.0 => n,
                        Some(n) => {
                            return Err(SyntaxError::new(
                                p.span_from(start),
                                format!("CI must be above 0 and below 100, got {n}"),
                            ));
                        }
                        None => return Err(p.unexpected(&["the confidence level"])),
                    };
                } else if p.eat_keyword("HIDENOTSIG") {
                    command.hide_not_significant = yes_no(p)?;
                } else if p.eat_keyword("SHOWREFLINE") {
                    command.reference_lines = yes_no(p)?;
                } else {
                    return Err(p.unexpected(&["ADJUST", "CI", "HIDENOTSIG", "SHOWREFLINE"]));
                }
                p.eat(Punct::Comma);
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
        } else if p.eat_keyword("PLOT") {
            p.eat(Punct::Eq);
            command.category_plot = false;
            command.cluster_plot = None;
            command.error_bar = false;
            command.importance = None;
            while !at_end(p) {
                if p.eat_keyword("CATEGORY") {
                    command.category_plot = true;
                } else if p.eat_keyword("CLUSTER") {
                    let mut pie = false;
                    if p.eat(Punct::LParen) {
                        if !p.eat_keyword("TYPE") {
                            return Err(p.unexpected(&["TYPE"]));
                        }
                        p.eat(Punct::Eq);
                        pie = if p.eat_keyword("PIE") {
                            true
                        } else if p.eat_keyword("BAR") {
                            false
                        } else {
                            return Err(p.unexpected(&["BAR", "PIE"]));
                        };
                        p.expect(Punct::RParen)?;
                    }
                    command.cluster_plot = Some(pie);
                } else if p.eat_keyword("ERRORBAR") {
                    command.error_bar = true;
                } else if p.eat_keyword("IMPORTANCE") {
                    let mut importance = AimImportance {
                        by_variable: false,
                        pvalue: false,
                    };
                    if p.eat(Punct::LParen) {
                        while !p.eat(Punct::RParen) {
                            if p.eat_word("X") {
                                p.eat(Punct::Eq);
                                importance.by_variable = if p.eat_keyword("VARIABLE") {
                                    true
                                } else if p.eat_keyword("GROUP") {
                                    false
                                } else {
                                    return Err(p.unexpected(&["GROUP", "VARIABLE"]));
                                };
                            } else if p.eat_word("Y") {
                                p.eat(Punct::Eq);
                                importance.pvalue = if p.eat_keyword("PVALUE") {
                                    true
                                } else if p.eat_keyword("TEST") {
                                    false
                                } else {
                                    return Err(p.unexpected(&["TEST", "PVALUE"]));
                                };
                            } else {
                                return Err(p.unexpected(&["X", "Y", "')'"]));
                            }
                            p.eat(Punct::Comma);
                        }
                    }
                    command.importance = Some(importance);
                } else {
                    return Err(p.unexpected(&["CATEGORY", "CLUSTER", "ERRORBAR", "IMPORTANCE"]));
                }
                p.eat(Punct::Comma);
            }
        } else {
            return Err(p.subcommand_error("AIM", &[]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    if command.categorical.is_empty() && command.continuous.is_empty() {
        return Err(SyntaxError::new(
            p.span(),
            "AIM needs a CATEGORICAL or a CONTINUOUS subcommand with variables",
        ));
    }
    Ok(Command::Aim(Box::new(command)))
}

/// The criterion that ranks categorical predictors of a categorical target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ranking {
    PearsonChiSquare,
    LikelihoodRatio,
    CramersV,
    Lambda,
}

const RANKINGS: [(&str, Ranking); 4] = [
    ("PEARSONCHISQ", Ranking::PearsonChiSquare),
    ("LRCHISQ", Ranking::LikelihoodRatio),
    ("CRAMERSV", Ranking::CramersV),
    ("LAMBDA", Ranking::Lambda),
];

/// The measurement level written after the dependent variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeasurementLevel {
    Scale,
    Ordinal,
    Nominal,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Selectpred {
    pub dependent: String,
    pub level: Option<MeasurementLevel>,
    /// BY and WITH: None when the keyword is not given, an empty list when it is given alone.
    pub factors: Option<Vec<VarItem>>,
    pub covariates: Option<Vec<VarItem>>,
    pub except: Vec<VarItem>,
    pub screening: bool,
    pub pct_missing: f64,
    pub pct_equal: f64,
    pub pct_unequal: f64,
    pub min_sd: f64,
    pub min_cv: f64,
    /// None for SIZE=AUTO.
    pub size: Option<usize>,
    pub p_cutoff: f64,
    pub ranking: Ranking,
    /// Minutes; 0 turns the timer off.
    pub timer: f64,
    pub show_unselected: usize,
    pub include: bool,
    pub print_cps: bool,
    pub print_excluded: bool,
    pub print_selected: bool,
    pub print_summary: bool,
    /// SUMMARY(...): further criteria shown next to the one that ranks.
    pub summary_criteria: Vec<Ranking>,
    pub plot: bool,
}

/// `KEYWORD=`: the equals sign is required.
fn equals(p: &mut Parser) -> PResult<()> {
    p.expect(Punct::Eq)
}

fn number_in(p: &mut Parser, what: &str, ok: impl Fn(f64) -> bool, rule: &str) -> PResult<f64> {
    equals(p)?;
    let start = p.pos;
    match p.number() {
        Some(n) if ok(n) => Ok(n),
        Some(n) => Err(SyntaxError::new(
            p.span_from(start),
            format!("{what} must be {rule}, got {n}"),
        )),
        None => Err(p.unexpected(&[what])),
    }
}

/// The keyword in `table` that is next, written in full.
fn word_of<T: Copy>(p: &mut Parser, table: &[(&str, T)]) -> Option<T> {
    table.iter().find(|(w, _)| p.eat_word(w)).map(|(_, t)| *t)
}

/// Records `keyword` as given in this subcommand; a second time is an error.
fn once(p: &Parser, seen: &mut Vec<String>, keyword: &str, start: usize) -> PResult<()> {
    if seen.iter().any(|s| s == keyword) {
        return Err(SyntaxError::new(
            p.span_from(start),
            format!("{keyword} is given twice in the same subcommand"),
        ));
    }
    seen.push(keyword.to_string());
    Ok(())
}

pub(crate) fn selectpred(p: &mut Parser) -> PResult<Command> {
    let dependent = p.name("the dependent variable")?;
    let mut level = None;
    if p.eat(Punct::LParen) {
        if !p.eat_word("MLEVEL") {
            return Err(p.unexpected(&["MLEVEL"]));
        }
        equals(p)?;
        level = Some(if p.eat_word("S") {
            MeasurementLevel::Scale
        } else if p.eat_word("O") {
            MeasurementLevel::Ordinal
        } else if p.eat_word("N") {
            MeasurementLevel::Nominal
        } else {
            return Err(p.unexpected(&["S", "O", "N"]));
        });
        p.expect(Punct::RParen)?;
    }
    let mut factors = None;
    let mut covariates = None;
    if p.eat_word("BY") {
        factors = Some(match at_end(p) || p.at_keyword("WITH") {
            true => Vec::new(),
            false => list_until(p, &["WITH"])?,
        });
    }
    if p.eat_word("WITH") {
        covariates = Some(match at_end(p) {
            true => Vec::new(),
            false => p.var_list()?,
        });
    }
    if !at_end(p) {
        return Err(p.unexpected(&["BY", "WITH", "'/'", "end of command"]));
    }
    let mut command = Selectpred {
        dependent,
        level,
        factors,
        covariates,
        except: Vec::new(),
        screening: true,
        pct_missing: 70.0,
        pct_equal: 95.0,
        pct_unequal: 90.0,
        min_sd: 0.0,
        min_cv: 0.001,
        size: None,
        p_cutoff: 0.05,
        ranking: Ranking::PearsonChiSquare,
        timer: 5.0,
        show_unselected: 0,
        include: false,
        print_cps: true,
        print_excluded: true,
        print_selected: true,
        print_summary: true,
        summary_criteria: Vec::new(),
        plot: false,
    };
    let mut subcommands: Vec<String> = Vec::new();
    let percent = |n: f64| n > 0.0 && n <= 100.0;
    let percent_rule = "above 0 and at most 100";
    while p.eat(Punct::Slash) {
        if at_end(p) {
            continue;
        }
        let start = p.pos;
        let Some(TokenKind::Word(word)) = p.peek() else {
            return Err(p.unexpected(&["a subcommand name"]));
        };
        let name = word.to_uppercase();
        let known = [
            "EXCEPT",
            "SCREENING",
            "CRITERIA",
            "MISSING",
            "PRINT",
            "PLOT",
        ];
        if !known.contains(&name.as_str()) {
            return Err(p.subcommand_error("SELECTPRED", &[]));
        }
        p.pos += 1;
        if at_end(p) {
            // an empty subcommand is not honored
            continue;
        }
        once(p, &mut subcommands, &name, start)?;
        let mut seen: Vec<String> = Vec::new();
        match name.as_str() {
            "EXCEPT" => {
                if !p.eat_word("VARIABLES") {
                    return Err(p.unexpected(&["VARIABLES"]));
                }
                equals(p)?;
                command.except = p.var_list()?;
            }
            "SCREENING" => {
                while !at_end(p) {
                    let start = p.pos;
                    let Some(TokenKind::Word(w)) = p.peek() else {
                        return Err(p.unexpected(&["a SCREENING keyword"]));
                    };
                    let keyword = w.to_uppercase();
                    p.pos += 1;
                    match keyword.as_str() {
                        "STATUS" => {
                            equals(p)?;
                            command.screening = if p.eat_word("ON") {
                                true
                            } else if p.eat_word("OFF") {
                                false
                            } else {
                                return Err(p.unexpected(&["ON", "OFF"]));
                            };
                        }
                        "PCTMISSING" => {
                            command.pct_missing = number_in(p, "PCTMISSING", percent, percent_rule)?
                        }
                        "PCTEQUAL" => {
                            command.pct_equal = number_in(p, "PCTEQUAL", percent, percent_rule)?
                        }
                        "PCTUNEQUAL" => {
                            command.pct_unequal = number_in(p, "PCTUNEQUAL", percent, percent_rule)?
                        }
                        "STDDEV" => {
                            command.min_sd = number_in(p, "STDDEV", |n| n >= 0.0, "at least 0")?
                        }
                        "CV" => command.min_cv = number_in(p, "CV", |n| n > 0.0, "above 0")?,
                        _ => {
                            p.pos -= 1;
                            return Err(p.unexpected(&[
                                "STATUS",
                                "PCTMISSING",
                                "PCTEQUAL",
                                "PCTUNEQUAL",
                                "STDDEV",
                                "CV",
                            ]));
                        }
                    }
                    once(p, &mut seen, &keyword, start)?;
                }
            }
            "CRITERIA" => {
                while !at_end(p) {
                    let start = p.pos;
                    let Some(TokenKind::Word(w)) = p.peek() else {
                        return Err(p.unexpected(&["a CRITERIA keyword"]));
                    };
                    let keyword = w.to_uppercase();
                    p.pos += 1;
                    match keyword.as_str() {
                        "SIZE" => {
                            equals(p)?;
                            command.size = if p.eat_word("AUTO") {
                                None
                            } else {
                                let n = number_in_bare(p, "SIZE")?;
                                if n.fract() != 0.0 || n < 1.0 {
                                    return Err(SyntaxError::new(
                                        p.span_from(start),
                                        format!(
                                            "SIZE must be AUTO or a whole number of at least 1, got {n}"
                                        ),
                                    ));
                                }
                                Some(n as usize)
                            };
                        }
                        "PCUTOFF" => {
                            command.p_cutoff = number_in(
                                p,
                                "PCUTOFF",
                                |n| n > 0.0 && n <= 1.0,
                                "above 0 and at most 1",
                            )?
                        }
                        "RANKING" => {
                            equals(p)?;
                            command.ranking = word_of(p, &RANKINGS).ok_or_else(|| {
                                p.unexpected(&["PEARSONCHISQ", "LRCHISQ", "CRAMERSV", "LAMBDA"])
                            })?;
                        }
                        "TIMER" => {
                            command.timer = number_in(p, "TIMER", |n| n >= 0.0, "at least 0")?
                        }
                        "SHOWUNSELECTED" => {
                            let n = number_in(
                                p,
                                "SHOWUNSELECTED",
                                |n| n >= 0.0 && n.fract() == 0.0 && n < 1e9,
                                "a whole number of at least 0",
                            )?;
                            command.show_unselected = n as usize;
                        }
                        _ => {
                            p.pos -= 1;
                            return Err(p.unexpected(&[
                                "SIZE",
                                "PCUTOFF",
                                "RANKING",
                                "TIMER",
                                "SHOWUNSELECTED",
                            ]));
                        }
                    }
                    once(p, &mut seen, &keyword, start)?;
                }
            }
            "MISSING" => {
                if !p.eat_word("USERMISSING") {
                    return Err(p.unexpected(&["USERMISSING"]));
                }
                equals(p)?;
                command.include = if p.eat_word("INCLUDE") {
                    true
                } else if p.eat_word("EXCLUDE") {
                    false
                } else {
                    return Err(p.unexpected(&["EXCLUDE", "INCLUDE"]));
                };
            }
            "PRINT" => {
                command.print_cps = false;
                command.print_excluded = false;
                command.print_selected = false;
                command.print_summary = false;
                let mut none = false;
                while !at_end(p) {
                    let start = p.pos;
                    let keyword = match p.peek() {
                        Some(TokenKind::Word(w)) => w.to_uppercase(),
                        _ => String::new(),
                    };
                    match keyword.as_str() {
                        "CPS" => command.print_cps = true,
                        "EXCLUDED" => command.print_excluded = true,
                        "SELECTED" => command.print_selected = true,
                        "NONE" => none = true,
                        "SUMMARY" => command.print_summary = true,
                        _ => {
                            return Err(
                                p.unexpected(&["CPS", "EXCLUDED", "SELECTED", "SUMMARY", "NONE"])
                            );
                        }
                    }
                    p.pos += 1;
                    if keyword == "SUMMARY" && p.eat(Punct::LParen) {
                        while !p.eat(Punct::RParen) {
                            let at = p.pos;
                            let criterion = word_of(p, &RANKINGS).ok_or_else(|| {
                                p.unexpected(&[
                                    "PEARSONCHISQ",
                                    "LRCHISQ",
                                    "CRAMERSV",
                                    "LAMBDA",
                                    "')'",
                                ])
                            })?;
                            if command.summary_criteria.contains(&criterion) {
                                return Err(SyntaxError::new(
                                    p.span_from(at),
                                    "a criterion is given twice in SUMMARY",
                                ));
                            }
                            command.summary_criteria.push(criterion);
                        }
                    }
                    once(p, &mut seen, &keyword, start)?;
                }
                if none && seen.len() > 1 {
                    return Err(SyntaxError::new(
                        p.span_from(start),
                        "PRINT NONE cannot be given with other keywords",
                    ));
                }
            }
            "PLOT" => {
                if !p.eat_word("SUMMARY") {
                    return Err(p.unexpected(&["SUMMARY"]));
                }
                command.plot = true;
            }
            _ => unreachable!("listed above"),
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    Ok(Command::Selectpred(Box::new(command)))
}

fn number_in_bare(p: &mut Parser, what: &str) -> PResult<f64> {
    p.number().ok_or_else(|| p.unexpected(&[what]))
}
