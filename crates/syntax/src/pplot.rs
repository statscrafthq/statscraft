//! Parsers of PPLOT (probability plots of variables against a test distribution) and ROC (the
//! receiver operating characteristic curve of test variables against a state variable).
//!
//! PPLOT rules from the Command Syntax Reference:
//! - `[VARIABLES=] varlist` comes first, or APPLY; VARIABLES is given once. Every other
//!   subcommand may repeat and the last one counts. NPPLOT is another name for the command.
//! - DISTRIBUTION: NORMAL, EXPONENTIAL, WEIBULL (also written WEIBUL), PARETO, LNORMAL, BETA,
//!   GAMMA, LOGISTIC, LAPLACE, UNIFORM, HNORMAL with their parameters in parentheses or none
//!   (estimated from the data); CHI and STUDENT need their degrees of freedom. Without a
//!   keyword: NORMAL.
//! - FRACTION: BLOM, RANKIT, TUKEY, VW; of several the first counts. TIES: MEAN, LOW, HIGH,
//!   BREAK. TYPE: Q-Q or P-P. PLOT: BOTH, NORMAL, DETRENDED.
//! - STANDARDIZE or NOSTANDARDIZE, LN or NOLOG: the last one counts. DIFF[=n] and SDIFF[=n]
//!   default to 1; PERIOD=n.
//! - APPLY [='name']: the specifications of the previous PPLOT, overridden by the
//!   subcommands given now.
//!
//! ROC rules from the Command Syntax Reference:
//! - `varlist BY state(value)`: numeric test variables; the state variable of any type and
//!   the value that marks the positive state.
//! - MISSING: EXCLUDE or INCLUDE. CRITERIA: CUTOFF(INCLUDE | EXCLUDE), TESTPOS(LARGE |
//!   SMALL), CI(n) with 0 < n < 100, DISTRIBUTION(FREE | NEGEXPO). PLOT: CURVE[(REFERENCE)]
//!   or NONE. PRINT: SE[(MODELQUALITY)] and COORDINATES. A repeated subcommand or keyword:
//!   the last one counts.

use crate::ast::{Command, RankFraction, RankTies, Value, VarItem};
use crate::error::SyntaxError;
use crate::glm::list_until;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::{at_end, names};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestDistribution {
    Normal,
    Exponential,
    Weibull,
    Pareto,
    Lognormal,
    Beta,
    Gamma,
    Logistic,
    Laplace,
    Uniform,
    HalfNormal,
    ChiSquare,
    StudentT,
}

impl TestDistribution {
    /// The number of parameters the distribution takes.
    pub fn parameters(self) -> usize {
        match self {
            Self::Exponential | Self::HalfNormal | Self::ChiSquare | Self::StudentT => 1,
            _ => 2,
        }
    }
}

const DISTRIBUTIONS: [(&str, TestDistribution); 14] = [
    ("NORMAL", TestDistribution::Normal),
    ("EXPONENTIAL", TestDistribution::Exponential),
    ("WEIBULL", TestDistribution::Weibull),
    ("WEIBUL", TestDistribution::Weibull),
    ("PARETO", TestDistribution::Pareto),
    ("LNORMAL", TestDistribution::Lognormal),
    ("BETA", TestDistribution::Beta),
    ("GAMMA", TestDistribution::Gamma),
    ("LOGISTIC", TestDistribution::Logistic),
    ("LAPLACE", TestDistribution::Laplace),
    ("UNIFORM", TestDistribution::Uniform),
    ("HNORMAL", TestDistribution::HalfNormal),
    ("CHI", TestDistribution::ChiSquare),
    ("STUDENT", TestDistribution::StudentT),
];

/// Which plots PLOT asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PplotPlot {
    Both,
    Normal,
    Detrended,
}

/// PPLOT as written: None (or an empty list) for what the command leaves out, so that APPLY
/// can fill it in from the previous PPLOT.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Pplot {
    pub variables: Vec<VarItem>,
    /// APPLY: Some(None) for the previous PPLOT, Some(Some(name)) for a named model.
    pub apply: Option<Option<String>>,
    /// The distribution and its parameters; no parameters when they are to be estimated.
    pub distribution: Option<(TestDistribution, Vec<f64>)>,
    pub fraction: Option<RankFraction>,
    /// BREAK is `Condense`.
    pub ties: Option<RankTies>,
    pub standardize: Option<bool>,
    /// TYPE: true for P-P, false for Q-Q.
    pub pp: Option<bool>,
    pub plot: Option<PplotPlot>,
    pub diff: Option<u32>,
    pub sdiff: Option<u32>,
    pub period: Option<u32>,
    pub ln: Option<bool>,
}

/// `[=] n` with n a positive whole number, or `default` when no number follows.
fn count(p: &mut Parser, default: Option<u32>, what: &str) -> PResult<u32> {
    p.eat(Punct::Eq);
    let start = p.pos;
    match p.number() {
        Some(n) if n >= 1.0 && n.fract() == 0.0 && n <= f64::from(u32::MAX) => Ok(n as u32),
        Some(n) => Err(SyntaxError::new(
            p.span_from(start),
            format!("{what} takes a positive whole number, got {n}"),
        )),
        None => default.ok_or_else(|| p.unexpected(&["a positive whole number"])),
    }
}

/// `[=] KEYWORD` from `choices`, then the end of the subcommand.
fn choice<T: Copy>(p: &mut Parser, choices: &[(&'static str, T)]) -> PResult<T> {
    p.eat(Punct::Eq);
    let Some((_, value)) = choices.iter().find(|(k, _)| p.at_keyword(k)) else {
        return Err(p.unexpected(&names(choices)));
    };
    p.pos += 1;
    Ok(*value)
}

fn distribution(p: &mut Parser) -> PResult<Option<(TestDistribution, Vec<f64>)>> {
    p.eat(Punct::Eq);
    if at_end(p) {
        return Ok(Some((TestDistribution::Normal, Vec::new())));
    }
    let start = p.pos;
    let kind = choice(p, &DISTRIBUTIONS)?;
    let mut parameters = Vec::new();
    if p.eat(Punct::LParen) {
        while !p.eat(Punct::RParen) {
            let Some(n) = p.number() else {
                return Err(p.unexpected(&["a number", "')'"]));
            };
            parameters.push(n);
            p.eat(Punct::Comma);
        }
    }
    let wanted = kind.parameters();
    let df = matches!(
        kind,
        TestDistribution::ChiSquare | TestDistribution::StudentT
    );
    if parameters.len() != wanted && (df || !parameters.is_empty()) {
        let message = match df {
            true => "CHI and STUDENT need their degrees of freedom in parentheses".to_string(),
            false => format!(
                "this distribution takes {wanted} parameter{} or none (to estimate them)",
                if wanted == 1 { "" } else { "s" }
            ),
        };
        return Err(SyntaxError::new(p.span_from(start), message));
    }
    let ok = match (kind, parameters.as_slice()) {
        (_, []) => true,
        (TestDistribution::Normal | TestDistribution::Logistic | TestDistribution::Laplace, p) => {
            p[1] > 0.0
        }
        (TestDistribution::Uniform, p) => p[0] < p[1],
        (_, p) => p.iter().all(|v| *v > 0.0),
    };
    if !ok {
        let message = match kind {
            TestDistribution::Uniform => "UNIFORM needs a minimum below its maximum",
            TestDistribution::Normal | TestDistribution::Logistic | TestDistribution::Laplace => {
                "the scale parameter (the second) must be positive"
            }
            _ => "the parameters of this distribution must be positive",
        };
        return Err(SyntaxError::new(p.span_from(start), message));
    }
    Ok(Some((kind, parameters)))
}

/// `Q-Q` or `P-P`.
fn plot_type(p: &mut Parser) -> PResult<bool> {
    p.eat(Punct::Eq);
    for (letter, pp) in [("Q", false), ("P", true)] {
        if matches!(p.peek(), Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case(letter))
            && p.peek_at(1) == Some(&TokenKind::Punct(Punct::Minus))
            && matches!(p.peek_at(2), Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case(letter))
        {
            p.pos += 3;
            return Ok(pp);
        }
    }
    Err(p.unexpected(&["Q-Q", "P-P"]))
}

pub(crate) fn pplot(p: &mut Parser) -> PResult<Command> {
    let mut command = Pplot::default();
    let mut first = true;
    loop {
        if first && p.at(Punct::Slash) {
            first = false;
        }
        if !first && !p.eat(Punct::Slash) {
            break;
        }
        let start = p.pos;
        if p.eat_keyword("VARIABLES") {
            if !command.variables.is_empty() {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    "VARIABLES is given only once",
                ));
            }
            p.eat(Punct::Eq);
            command.variables = p.var_list()?;
        } else if p.eat_keyword("APPLY") {
            p.eat(Punct::Eq);
            command.apply = Some(match p.peek() {
                Some(TokenKind::Str(_)) => Some(p.string("a model name")?),
                _ => None,
            });
        } else if first {
            command.variables = p.var_list()?;
        } else if p.eat_keyword("DISTRIBUTION") {
            command.distribution = distribution(p)?;
        } else if p.eat_keyword("FRACTION") {
            const FRACTIONS: [(&str, RankFraction); 4] = [
                ("BLOM", RankFraction::Blom),
                ("RANKIT", RankFraction::Rankit),
                ("TUKEY", RankFraction::Tukey),
                ("VW", RankFraction::Vw),
            ];
            let fraction = choice(p, &FRACTIONS)?;
            // of several formulas the first counts
            while matches!(
                p.peek(),
                Some(TokenKind::Word(_)) | Some(TokenKind::Punct(Punct::Comma))
            ) {
                p.pos += 1;
            }
            command.fraction = Some(fraction);
        } else if p.eat_keyword("TIES") {
            const TIES: [(&str, RankTies); 4] = [
                ("MEAN", RankTies::Mean),
                ("LOW", RankTies::Low),
                ("HIGH", RankTies::High),
                ("BREAK", RankTies::Condense),
            ];
            command.ties = Some(choice(p, &TIES)?);
        } else if p.eat_keyword("STANDARDIZE") {
            command.standardize = Some(true);
        } else if p.eat_keyword("NOSTANDARDIZE") {
            command.standardize = Some(false);
        } else if p.eat_keyword("TYPE") {
            command.pp = Some(plot_type(p)?);
        } else if p.eat_keyword("PLOT") {
            const PLOTS: [(&str, PplotPlot); 3] = [
                ("BOTH", PplotPlot::Both),
                ("NORMAL", PplotPlot::Normal),
                ("DETRENDED", PplotPlot::Detrended),
            ];
            command.plot = Some(choice(p, &PLOTS)?);
        } else if p.eat_keyword("DIFF") {
            command.diff = Some(count(p, Some(1), "DIFF")?);
        } else if p.eat_keyword("SDIFF") {
            command.sdiff = Some(count(p, Some(1), "SDIFF")?);
        } else if p.eat_keyword("PERIOD") {
            command.period = Some(count(p, None, "PERIOD")?);
        } else if p.eat_word("LN") {
            command.ln = Some(true);
        } else if p.eat_keyword("NOLOG") {
            command.ln = Some(false);
        } else {
            return Err(p.subcommand_error("PPLOT", &["TEMPLATE"]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
        first = false;
    }
    p.finish(&["'/'"])?;
    if command.variables.is_empty() && command.apply.is_none() {
        return Err(SyntaxError::new(
            p.span(),
            "PPLOT needs VARIABLES, or APPLY to use the previous PPLOT's",
        ));
    }
    Ok(Command::Pplot(Box::new(command)))
}

#[derive(Debug, Clone, PartialEq)]
pub struct Roc {
    pub tests: Vec<VarItem>,
    pub state: String,
    /// The value of the state variable that marks the positive state.
    pub positive: Value,
    /// `/MISSING=INCLUDE`
    pub include: bool,
    /// CUTOFF(EXCLUDE): a value equal to the cutoff is negative.
    pub exclude_cutoff: bool,
    /// TESTPOS(SMALL): smaller test values point to the positive state.
    pub small_positive: bool,
    /// The confidence level in percent.
    pub ci: f64,
    /// DISTRIBUTION(NEGEXPO): the bi-negative exponential standard error.
    pub negexpo: bool,
    pub curve: bool,
    pub reference: bool,
    pub se: bool,
    pub model_quality: bool,
    pub coordinates: bool,
}

/// `(KEYWORD)` from `choices`.
fn in_parens<T: Copy>(p: &mut Parser, choices: &[(&'static str, T)]) -> PResult<T> {
    p.expect(Punct::LParen)?;
    let value = choice(p, choices)?;
    p.expect(Punct::RParen)?;
    Ok(value)
}

pub(crate) fn roc(p: &mut Parser) -> PResult<Command> {
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
    let mut command = Roc {
        tests,
        state,
        positive,
        include: false,
        exclude_cutoff: false,
        small_positive: false,
        ci: 95.0,
        negexpo: false,
        curve: true,
        reference: false,
        se: false,
        model_quality: false,
        coordinates: false,
    };
    while p.eat(Punct::Slash) {
        if p.eat_keyword("MISSING") {
            command.include = choice(p, &[("EXCLUDE", false), ("INCLUDE", true)])?;
        } else if p.eat_keyword("CRITERIA") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                if p.eat_keyword("CUTOFF") {
                    command.exclude_cutoff =
                        in_parens(p, &[("INCLUDE", false), ("EXCLUDE", true)])?;
                } else if p.eat_keyword("TESTPOS") {
                    command.small_positive = in_parens(p, &[("LARGE", false), ("SMALL", true)])?;
                } else if p.eat_keyword("DISTRIBUTION") {
                    command.negexpo = in_parens(p, &[("FREE", false), ("NEGEXPO", true)])?;
                } else if p.eat_word("CI") {
                    p.expect(Punct::LParen)?;
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
                    p.expect(Punct::RParen)?;
                    command.ci = level;
                } else {
                    return Err(p.unexpected(&["CUTOFF", "TESTPOS", "CI", "DISTRIBUTION"]));
                }
                p.eat(Punct::Comma);
            }
        } else if p.eat_keyword("PLOT") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                if p.eat_keyword("CURVE") {
                    command.curve = true;
                    command.reference = false;
                    if p.eat(Punct::LParen) {
                        if !p.eat_keyword("REFERENCE") {
                            return Err(p.unexpected(&["REFERENCE"]));
                        }
                        p.expect(Punct::RParen)?;
                        command.reference = true;
                    }
                } else if p.eat_keyword("NONE") {
                    command.curve = false;
                } else {
                    return Err(p.unexpected(&["CURVE", "NONE"]));
                }
                p.eat(Punct::Comma);
            }
        } else if p.eat_keyword("PRINT") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                if p.eat_word("SE") {
                    command.se = true;
                    if p.eat(Punct::LParen) {
                        if !p.eat_keyword("MODELQUALITY") {
                            return Err(p.unexpected(&["MODELQUALITY"]));
                        }
                        p.expect(Punct::RParen)?;
                        command.model_quality = true;
                    }
                } else if p.eat_keyword("COORDINATES") {
                    command.coordinates = true;
                } else {
                    return Err(p.unexpected(&["SE", "COORDINATES"]));
                }
                p.eat(Punct::Comma);
            }
        } else {
            return Err(p.subcommand_error("ROC", &[]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    Ok(Command::Roc(Box::new(command)))
}
