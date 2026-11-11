//! Parser of RELIABILITY: reliability of an additive scale of items.
//!
//! Rules from the Command Syntax Reference (RELIABILITY): VARIABLES comes first; SCALE(label)
//! names the items of the scale among them (default: all, labelled ALL); MODEL is ALPHA
//! (default), SPLIT[(n)] with n items in the second part, GUTTMAN, PARALLEL or STRICTPARALLEL;
//! STATISTICS and SUMMARY add up when given twice; FRIEDMAN and COCHRAN count only with ANOVA;
//! ICC defaults to MODEL(MIXED) TYPE(CONSISTENCY) CIN=95 TESTVAL=0; MISSING is EXCLUDE or
//! INCLUDE.

use crate::ast::{Command, VarItem};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::{at_end, keyword_error};

#[derive(Debug, Clone, PartialEq)]
pub struct Reliability {
    pub variables: Vec<VarItem>,
    /// `/SCALE(label)=varlist`; None is every variable, labelled ALL.
    pub scale: Option<(String, Vec<VarItem>)>,
    pub model: ReliabilityModel,
    pub descriptive: bool,
    /// `SCALE` on STATISTICS: the mean and variance of the scale.
    pub scale_statistics: bool,
    pub correlations: bool,
    pub covariances: bool,
    pub anova: Option<AnovaTest>,
    pub tukey: bool,
    pub hotelling: bool,
    pub summary: ReliabilitySummary,
    pub icc: Option<Icc>,
    /// `/METHOD=COVARIANCE`
    pub covariance_method: bool,
    /// `/MISSING=INCLUDE`
    pub include: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReliabilityModel {
    Alpha,
    /// The number of items in the second part, if given.
    Split(Option<u32>),
    Guttman,
    Parallel,
    StrictParallel,
}

/// The test of the repeated measures ANOVA table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnovaTest {
    F,
    Friedman,
    Cochran,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReliabilitySummary {
    pub means: bool,
    pub variances: bool,
    pub covariances: bool,
    pub correlations: bool,
    pub total: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Icc {
    pub model: IccModel,
    /// `TYPE(ABSOLUTE)`; the default is CONSISTENCY.
    pub absolute: bool,
    /// `CIN`: the confidence level in percent.
    pub level: f64,
    /// `TESTVAL`: the value the coefficients are tested against.
    pub test_value: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IccModel {
    OneWay,
    Mixed,
    Random,
}

fn icc(p: &mut Parser) -> PResult<Icc> {
    p.eat(Punct::Eq);
    let mut icc = Icc {
        model: IccModel::Mixed,
        absolute: false,
        level: 95.0,
        test_value: 0.0,
    };
    while !at_end(p) {
        if p.eat_keyword("MODEL") {
            p.expect(Punct::LParen)?;
            icc.model = if p.eat_keyword("ONEWAY") {
                IccModel::OneWay
            } else if p.eat_keyword("MIXED") {
                IccModel::Mixed
            } else if p.eat_keyword("RANDOM") {
                IccModel::Random
            } else {
                return Err(p.unexpected(&["ONEWAY", "MIXED", "RANDOM"]));
            };
            p.expect(Punct::RParen)?;
        } else if p.eat_keyword("TYPE") {
            p.expect(Punct::LParen)?;
            icc.absolute = if p.eat_keyword("CONSISTENCY") {
                false
            } else if p.eat_keyword("ABSOLUTE") {
                true
            } else {
                return Err(p.unexpected(&["CONSISTENCY", "ABSOLUTE"]));
            };
            p.expect(Punct::RParen)?;
        } else if p.at_keyword("CIN") || p.at_keyword("TESTVAL") {
            let cin = p.at_keyword("CIN");
            p.pos += 1;
            let parenthesized = p.eat(Punct::LParen);
            if !parenthesized {
                p.expect(Punct::Eq)?;
            }
            let start = p.pos;
            let Some(value) = p.number() else {
                return Err(p.unexpected(&["a number"]));
            };
            if parenthesized {
                p.expect(Punct::RParen)?;
            }
            if cin {
                if !(value > 0.0 && value < 100.0) {
                    let message = format!("CIN must lie between 0 and 100, got {value}");
                    return Err(SyntaxError::new(p.span_from(start), message));
                }
                icc.level = value;
            } else {
                if !(0.0..1.0).contains(&value) {
                    let message = format!("TESTVAL must lie from 0 up to 1, got {value}");
                    return Err(SyntaxError::new(p.span_from(start), message));
                }
                icc.test_value = value;
            }
        } else {
            return Err(p.unexpected(&["MODEL", "TYPE", "CIN", "TESTVAL"]));
        }
        p.eat(Punct::Comma);
    }
    Ok(icc)
}

pub(crate) fn reliability(p: &mut Parser) -> PResult<Command> {
    p.eat(Punct::Slash);
    if !p.eat_keyword("VARIABLES") {
        return Err(p
            .unexpected(&["VARIABLES"])
            .hint("RELIABILITY starts with VARIABLES=varlist"));
    }
    p.eat(Punct::Eq);
    let mut command = Reliability {
        variables: p.var_list()?,
        scale: None,
        model: ReliabilityModel::Alpha,
        descriptive: false,
        scale_statistics: false,
        correlations: false,
        covariances: false,
        anova: None,
        tukey: false,
        hotelling: false,
        summary: ReliabilitySummary::default(),
        icc: None,
        covariance_method: false,
        include: false,
    };
    let mut rank_test = None;
    while p.eat(Punct::Slash) {
        let start = p.pos;
        if p.eat_keyword("SCALE") {
            if command.scale.is_some() {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    "SCALE can be given only once",
                ));
            }
            p.expect(Punct::LParen)?;
            let label = match p.peek() {
                Some(TokenKind::Str(_)) => p.string("a label")?,
                _ => p.name("a label")?,
            };
            p.expect(Punct::RParen)?;
            p.eat(Punct::Eq);
            command.scale = Some((label, p.var_list()?));
        } else if p.eat_keyword("MODEL") {
            p.eat(Punct::Eq);
            command.model = if p.eat_keyword("ALPHA") {
                ReliabilityModel::Alpha
            } else if p.eat_keyword("SPLIT") {
                let mut n = None;
                if p.eat(Punct::LParen) {
                    let at = p.pos;
                    match p.number() {
                        Some(v) if v >= 1.0 && v.fract() == 0.0 => n = Some(v as u32),
                        Some(v) => {
                            return Err(SyntaxError::new(
                                p.span_from(at),
                                format!("SPLIT takes a positive whole number of items, got {v}"),
                            ));
                        }
                        None => return Err(p.unexpected(&["a number of items"])),
                    }
                    p.expect(Punct::RParen)?;
                }
                ReliabilityModel::Split(n)
            } else if p.eat_keyword("GUTTMAN") {
                ReliabilityModel::Guttman
            } else if p.eat_keyword("PARALLEL") {
                ReliabilityModel::Parallel
            } else if p.eat_keyword("STRICTPARALLEL") {
                ReliabilityModel::StrictParallel
            } else {
                const MODELS: [&str; 5] =
                    ["ALPHA", "SPLIT", "GUTTMAN", "PARALLEL", "STRICTPARALLEL"];
                return Err(keyword_error(p, "RELIABILITY /MODEL", &["OMEGA"], &MODELS));
            };
        } else if p.eat_keyword("STATISTICS") {
            p.eat(Punct::Eq);
            const KEYWORDS: [&str; 11] = [
                "DESCRIPTIVES",
                "SCALE",
                "CORRELATIONS",
                "COVARIANCES",
                "ANOVA",
                "FRIEDMAN",
                "COCHRAN",
                "TUKEY",
                "HOTELLING",
                "ALL",
                "DESCRIPTIVE",
            ];
            let mut any = false;
            while !at_end(p) {
                let Some(i) = KEYWORDS.iter().position(|k| p.at_keyword(k)) else {
                    return Err(p.unexpected(&KEYWORDS[..10]));
                };
                p.pos += 1;
                any = true;
                match i {
                    0 | 10 => command.descriptive = true,
                    1 => command.scale_statistics = true,
                    2 => command.correlations = true,
                    3 => command.covariances = true,
                    4 => {
                        command.anova.get_or_insert(AnovaTest::F);
                    }
                    5 => rank_test = Some(AnovaTest::Friedman),
                    6 => rank_test = Some(AnovaTest::Cochran),
                    7 => command.tukey = true,
                    8 => command.hotelling = true,
                    _ => {
                        command.descriptive = true;
                        command.scale_statistics = true;
                        command.correlations = true;
                        command.covariances = true;
                        command.anova.get_or_insert(AnovaTest::F);
                        command.tukey = true;
                        command.hotelling = true;
                    }
                }
                p.eat(Punct::Comma);
            }
            if !any {
                return Err(p.unexpected(&KEYWORDS[..10]));
            }
        } else if p.eat_keyword("SUMMARY") {
            p.eat(Punct::Eq);
            const KEYWORDS: [&str; 6] = [
                "MEANS",
                "VARIANCE",
                "COVARIANCES",
                "CORRELATIONS",
                "TOTAL",
                "ALL",
            ];
            let mut any = false;
            let s = &mut command.summary;
            while !at_end(p) {
                let Some(i) = KEYWORDS.iter().position(|k| p.at_keyword(k)) else {
                    return Err(p.unexpected(&KEYWORDS));
                };
                p.pos += 1;
                any = true;
                match i {
                    0 => s.means = true,
                    1 => s.variances = true,
                    2 => s.covariances = true,
                    3 => s.correlations = true,
                    4 => s.total = true,
                    _ => {
                        *s = ReliabilitySummary {
                            means: true,
                            variances: true,
                            covariances: true,
                            correlations: true,
                            total: true,
                        }
                    }
                }
                p.eat(Punct::Comma);
            }
            if !any {
                return Err(p.unexpected(&KEYWORDS));
            }
        } else if p.eat_keyword("ICC") {
            command.icc = Some(icc(p)?);
        } else if p.eat_keyword("METHOD") {
            p.eat(Punct::Eq);
            if !p.eat_keyword("COVARIANCE") {
                return Err(p.unexpected(&["COVARIANCE"]));
            }
            command.covariance_method = true;
        } else if p.eat_keyword("MISSING") {
            p.eat(Punct::Eq);
            if p.eat_keyword("INCLUDE") {
                command.include = true;
            } else if p.eat_keyword("EXCLUDE") {
                command.include = false;
            } else {
                return Err(p.unexpected(&["EXCLUDE", "INCLUDE"]));
            }
        } else if p.at_keyword("VARIABLES") {
            return Err(SyntaxError::new(
                p.span(),
                "VARIABLES can be given only once",
            ));
        } else {
            return Err(p.subcommand_error("RELIABILITY", &["MATRIX"]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    // FRIEDMAN and COCHRAN replace F only when ANOVA is asked for
    if let (Some(test), Some(anova)) = (rank_test, command.anova.as_mut()) {
        *anova = test;
    }
    Ok(Command::Reliability(Box::new(command)))
}
