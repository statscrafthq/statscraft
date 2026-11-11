//! NPAR TESTS and PARTIAL CORR: their syntax tree and parsers.

use crate::ast::{Command, TTestPairs, VarItem};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::{at_end, keywords, names};
use crate::stats::{pair_list, variables};
use crate::table::matches_keyword;

#[derive(Debug, Clone, PartialEq)]
pub struct NparTests {
    /// The test subcommands in the order written.
    pub tests: Vec<NparTest>,
    /// `/STATISTICS=DESCRIPTIVES`
    pub descriptives: bool,
    /// `/STATISTICS=QUARTILES`
    pub quartiles: bool,
    /// `/MISSING=LISTWISE`: cases missing on any variable of the command leave every test.
    pub listwise: bool,
    /// `/MISSING=INCLUDE`
    pub include: bool,
    /// `/METHOD=EXACT` or `/METHOD=MC`: exact significances where they are computed.
    pub exact: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NparTest {
    /// `/CHISQUARE=varlist [(lo, hi)]` with the expected frequencies of its `/EXPECTED`
    /// (empty for EQUAL).
    ChiSquare {
        variables: Vec<VarItem>,
        range: Option<(f64, f64)>,
        expected: Vec<f64>,
    },
    /// `/BINOMIAL[(p)]=varlist[(value[, value])]`
    Binomial {
        p: f64,
        variables: Vec<VarItem>,
        split: BinomialSplit,
    },
    /// `/RUNS(cut)=varlist`
    Runs {
        cut: RunsCut,
        variables: Vec<VarItem>,
    },
    /// `/K-S(distribution [parameters])=varlist`, the one-sample test.
    KolmogorovSmirnov {
        distribution: KsDistribution,
        parameters: Vec<f64>,
        variables: Vec<VarItem>,
    },
    /// Two related samples: `/WILCOXON`, `/SIGN`, `/MCNEMAR` and `/MH`.
    Paired { test: PairedTest, pairs: TTestPairs },
    /// k related samples: `/FRIEDMAN`, `/KENDALL` and `/COCHRAN`.
    Related {
        test: RelatedTest,
        variables: Vec<VarItem>,
    },
    /// Independent samples: `test=varlist BY var(value1, value2)`.
    Groups {
        test: GroupTest,
        variables: Vec<VarItem>,
        group: String,
        values: (f64, f64),
    },
}

/// How BINOMIAL forms its two categories.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinomialSplit {
    /// The variable is dichotomous; the first value met is the first category.
    Dichotomy,
    /// Values up to the cut point, and the values above it.
    Cut(f64),
    /// The first value, and the second.
    Values(f64, f64),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RunsCut {
    Mean,
    Median,
    Mode,
    Value(f64),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KsDistribution {
    Normal,
    Uniform,
    Poisson,
    Exponential,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PairedTest {
    Wilcoxon,
    Sign,
    McNemar,
    MarginalHomogeneity,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RelatedTest {
    Friedman,
    Kendall,
    Cochran,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GroupTest {
    MannWhitney,
    KolmogorovSmirnov,
    WaldWolfowitz,
    /// The number of cases trimmed from each end of the control group, when given.
    Moses(Option<f64>),
    KruskalWallis,
    /// The median given, else the median of the cases in the test.
    Median(Option<f64>),
    JonckheereTerpstra,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PartialCorr {
    pub lists: Vec<PartialList>,
    /// `/SIGNIFICANCE=ONETAIL`
    pub one_tailed: bool,
    /// `/STATISTICS=CORR`: the zero-order correlations.
    pub zero_order: bool,
    /// `/STATISTICS=BADCORR`: the zero-order correlations when one cannot be computed.
    pub bad_corr: bool,
    /// `/STATISTICS=DESCRIPTIVES`
    pub descriptives: bool,
    pub format: PartialFormat,
    /// `/MISSING=ANALYSIS`: each zero-order correlation from the cases valid on its pair.
    pub pairwise: bool,
    /// `/MISSING=INCLUDE`
    pub include: bool,
}

/// One variable list: `varlist [WITH varlist] BY controls [(orders)]`.
#[derive(Debug, Clone, PartialEq)]
pub struct PartialList {
    pub rows: Vec<VarItem>,
    pub columns: Option<Vec<VarItem>>,
    pub controls: Vec<VarItem>,
    /// The orders asked for; empty means as many as there are control variables.
    pub orders: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PartialFormat {
    Matrix,
    Condensed,
    Serial,
}

/// A test name written with a hyphen, such as M-W, as its two words.
const HYPHENATED: [(&str, &str, Hyphenated); 11] = [
    ("CHI", "SQUARE", Hyphenated::ChiSquare),
    ("K", "S", Hyphenated::Ks),
    ("KOLMOGOROV", "SMIRNOV", Hyphenated::Ks),
    ("M", "W", Hyphenated::Group(GroupTest::MannWhitney)),
    ("MANN", "WHITNEY", Hyphenated::Group(GroupTest::MannWhitney)),
    ("W", "W", Hyphenated::Group(GroupTest::WaldWolfowitz)),
    (
        "WALD",
        "WOLFOWITZ",
        Hyphenated::Group(GroupTest::WaldWolfowitz),
    ),
    ("K", "W", Hyphenated::Group(GroupTest::KruskalWallis)),
    (
        "KRUSKAL",
        "WALLIS",
        Hyphenated::Group(GroupTest::KruskalWallis),
    ),
    ("J", "T", Hyphenated::Group(GroupTest::JonckheereTerpstra)),
    (
        "JONCKHEERE",
        "TERPSTRA",
        Hyphenated::Group(GroupTest::JonckheereTerpstra),
    ),
];

#[derive(Clone, Copy)]
enum Hyphenated {
    ChiSquare,
    Ks,
    Group(GroupTest),
}

/// Takes a hyphenated test name such as `K-S` at the parser.
fn hyphenated(p: &mut Parser) -> Option<Hyphenated> {
    let word = |offset: usize| match p.peek_at(offset) {
        Some(TokenKind::Word(w)) => Some(w.as_str()),
        _ => None,
    };
    let minus = p.peek_at(1) == Some(&TokenKind::Punct(Punct::Minus));
    let (Some(first), true, Some(second)) = (word(0), minus, word(2)) else {
        return None;
    };
    let found = HYPHENATED
        .iter()
        .find(|(a, b, _)| first.eq_ignore_ascii_case(a) && second.eq_ignore_ascii_case(b));
    let (_, _, kind) = found?;
    p.pos += 3;
    Some(*kind)
}

const NPAR_SUBCOMMANDS: [&str; 18] = [
    "CHISQUARE",
    "EXPECTED",
    "BINOMIAL",
    "RUNS",
    "MCNEMAR",
    "SIGN",
    "WILCOXON",
    "MH",
    "COCHRAN",
    "FRIEDMAN",
    "KENDALL",
    "MOSES",
    "MEDIAN",
    "KS_SIM",
    "MISSING",
    "SAMPLE",
    "STATISTICS",
    "METHOD",
];

/// A number in parentheses or after '=', for settings such as CIN(99).
fn setting(p: &mut Parser) -> PResult<f64> {
    let parenthesized = p.eat(Punct::LParen);
    if !parenthesized {
        p.eat(Punct::Eq);
    }
    let value = p.number().ok_or_else(|| p.unexpected(&["a number"]))?;
    if parenthesized {
        p.expect(Punct::RParen)?;
    }
    Ok(value)
}

/// `(number [,] number)`; the second is optional when `one` allows it.
fn numbers_in_parens(p: &mut Parser, one: bool) -> PResult<(f64, Option<f64>)> {
    p.expect(Punct::LParen)?;
    let first = p.number().ok_or_else(|| p.unexpected(&["a number"]))?;
    p.eat(Punct::Comma);
    let second = p.number();
    if second.is_none() && !one {
        return Err(p.unexpected(&["a second number"]));
    }
    p.expect(Punct::RParen)?;
    Ok((first, second))
}

/// `varlist BY var(value1, value2)`
fn group_test(p: &mut Parser, test: GroupTest) -> PResult<NparTest> {
    p.eat(Punct::Eq);
    let variables = variables(p)?;
    if !p.eat_word("BY") {
        return Err(p.unexpected(&["BY"]));
    }
    let group = p.name("the grouping variable")?;
    let (first, second) = numbers_in_parens(p, false)?;
    Ok(NparTest::Groups {
        test,
        variables,
        group,
        values: (first, second.unwrap_or(first)),
    })
}

pub(crate) fn npar_tests(p: &mut Parser) -> PResult<Command> {
    let mut command = NparTests {
        tests: Vec::new(),
        descriptives: false,
        quartiles: false,
        listwise: false,
        include: false,
        exact: false,
    };
    let start = p.pos;
    let mut once: Vec<&str> = Vec::new();
    let mut only_once = |p: &Parser, name: &'static str| {
        if once.contains(&name) {
            let message = format!("{name} may be given only once in NPAR TESTS");
            return Err(SyntaxError::new(p.span_from(p.pos - 1), message));
        }
        once.push(name);
        Ok(())
    };
    p.eat(Punct::Slash);
    loop {
        if let Some(kind) = hyphenated(p) {
            let test = match kind {
                Hyphenated::ChiSquare => chi_square(p)?,
                Hyphenated::Ks if p.at(Punct::LParen) => one_sample_ks(p)?,
                Hyphenated::Ks => group_test(p, GroupTest::KolmogorovSmirnov)?,
                Hyphenated::Group(test) => group_test(p, test)?,
            };
            command.tests.push(test);
        } else if p.eat_keyword("CHISQUARE") {
            command.tests.push(chi_square(p)?);
        } else if p.eat_keyword("EXPECTED") {
            p.eat(Punct::Eq);
            let expected = expected(p)?;
            let last = command.tests.iter_mut().rev().find_map(|t| match t {
                NparTest::ChiSquare { expected, .. } => Some(expected),
                _ => None,
            });
            let Some(target) = last else {
                let message = "EXPECTED applies to the CHISQUARE before it, and there is none";
                return Err(SyntaxError::new(p.span_from(start), message));
            };
            *target = expected;
        } else if p.eat_keyword("BINOMIAL") {
            let mut prob = 0.5;
            if p.eat(Punct::LParen) {
                let at = p.pos;
                prob = p.number().ok_or_else(|| p.unexpected(&["a proportion"]))?;
                if !(prob > 0.0 && prob < 1.0) {
                    let message =
                        format!("the test proportion must lie between 0 and 1, got {prob}");
                    return Err(SyntaxError::new(p.span_from(at), message));
                }
                p.expect(Punct::RParen)?;
            }
            p.eat(Punct::Eq);
            let variables = variables(p)?;
            let split = if p.at(Punct::LParen) {
                match numbers_in_parens(p, true)? {
                    (cut, None) => BinomialSplit::Cut(cut),
                    (a, Some(b)) => BinomialSplit::Values(a, b),
                }
            } else {
                BinomialSplit::Dichotomy
            };
            command.tests.push(NparTest::Binomial {
                p: prob,
                variables,
                split,
            });
        } else if p.eat_keyword("RUNS") {
            p.expect(Punct::LParen)?;
            let cut = if p.eat_keyword("MEAN") {
                RunsCut::Mean
            } else if p.eat_keyword("MEDIAN") {
                RunsCut::Median
            } else if p.eat_keyword("MODE") {
                RunsCut::Mode
            } else if let Some(value) = p.number() {
                RunsCut::Value(value)
            } else {
                return Err(p.unexpected(&["MEAN", "MEDIAN", "MODE", "a number"]));
            };
            p.expect(Punct::RParen)?;
            p.eat(Punct::Eq);
            let variables = variables(p)?;
            command.tests.push(NparTest::Runs { cut, variables });
        } else if let Some(test) = [
            ("WILCOXON", PairedTest::Wilcoxon),
            ("SIGN", PairedTest::Sign),
            ("MCNEMAR", PairedTest::McNemar),
            ("MH", PairedTest::MarginalHomogeneity),
        ]
        .into_iter()
        .find_map(|(name, test)| p.eat_keyword(name).then_some(test))
        {
            p.eat(Punct::Eq);
            let pairs = pair_list(p)?;
            command.tests.push(NparTest::Paired { test, pairs });
        } else if let Some(test) = [
            ("FRIEDMAN", RelatedTest::Friedman),
            ("KENDALL", RelatedTest::Kendall),
            ("COCHRAN", RelatedTest::Cochran),
        ]
        .into_iter()
        .find_map(|(name, test)| p.eat_keyword(name).then_some(test))
        {
            p.eat(Punct::Eq);
            let variables = variables(p)?;
            command.tests.push(NparTest::Related { test, variables });
        } else if p.eat_keyword("MOSES") {
            let trim = if p.eat(Punct::LParen) {
                let n = p
                    .number()
                    .ok_or_else(|| p.unexpected(&["a number of cases"]))?;
                p.expect(Punct::RParen)?;
                Some(n)
            } else {
                None
            };
            command.tests.push(group_test(p, GroupTest::Moses(trim))?);
        } else if p.eat_keyword("MEDIAN") {
            let median = if p.eat(Punct::LParen) {
                let n = p
                    .number()
                    .ok_or_else(|| p.unexpected(&["a median value"]))?;
                p.expect(Punct::RParen)?;
                Some(n)
            } else {
                None
            };
            command
                .tests
                .push(group_test(p, GroupTest::Median(median))?);
        } else if p.eat_keyword("MISSING") {
            only_once(p, "MISSING")?;
            p.eat(Punct::Eq);
            const CHOICES: [(&str, &[u8]); 3] =
                [("ANALYSIS", &[0]), ("LISTWISE", &[1]), ("INCLUDE", &[2])];
            for choice in keywords(p, &CHOICES, |p| p.unexpected(&names(&CHOICES)))? {
                match choice {
                    0 => command.listwise = false,
                    1 => command.listwise = true,
                    _ => command.include = true,
                }
            }
        } else if p.eat_keyword("STATISTICS") {
            only_once(p, "STATISTICS")?;
            p.eat(Punct::Eq);
            if at_end(p) {
                command.descriptives = true;
            } else {
                const CHOICES: [(&str, &[u8]); 3] = [
                    ("DESCRIPTIVES", &[0]),
                    ("QUARTILES", &[1]),
                    ("ALL", &[0, 1]),
                ];
                for choice in keywords(p, &CHOICES, |p| p.unexpected(&names(&CHOICES)))? {
                    match choice {
                        0 => command.descriptives = true,
                        _ => command.quartiles = true,
                    }
                }
            }
        } else if p.eat_keyword("SAMPLE") {
            // sampling applies only when the cases do not fit in memory, which they always do
            only_once(p, "SAMPLE")?;
        } else if p.eat_keyword("KS_SIM") {
            // the Lilliefors significance is computed from its approximation, not simulated
            only_once(p, "KS_SIM")?;
            while !at_end(p) {
                if p.eat_keyword("CIN") || p.eat_keyword("SAMPLES") {
                    if p.at(Punct::LParen) || p.at(Punct::Eq) {
                        setting(p)?;
                    } else {
                        p.number();
                    }
                } else if !p.eat_keyword("NONORMAL") {
                    return Err(p.unexpected(&["CIN", "SAMPLES", "NONORMAL"]));
                }
            }
        } else if p.eat_keyword("METHOD") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                if p.eat_word("MC") || p.eat_keyword("EXACT") {
                    command.exact = true;
                } else if p.eat_keyword("CIN") || p.eat_keyword("SAMPLES") || p.eat_keyword("TIMER")
                {
                    setting(p)?;
                } else {
                    return Err(p.unexpected(&["MC", "EXACT", "CIN", "SAMPLES", "TIMER"]));
                }
            }
        } else {
            let mut error = p.subcommand_error("NPAR TESTS", &[]);
            if error.expected.is_empty() {
                error.expected = NPAR_SUBCOMMANDS.iter().map(|s| s.to_string()).collect();
                error
                    .expected
                    .extend(["K-S", "M-W", "W-W", "K-W", "J-T"].map(String::from));
            }
            return Err(error);
        }
        if !p.eat(Punct::Slash) {
            break;
        }
    }
    p.finish(&["'/'"])?;
    if command.tests.is_empty() {
        let message =
            "NPAR TESTS needs at least one test subcommand, such as /M-W=var BY group(1,2)";
        return Err(SyntaxError::new(p.span_from(start), message));
    }
    Ok(Command::NparTests(Box::new(command)))
}

fn chi_square(p: &mut Parser) -> PResult<NparTest> {
    p.eat(Punct::Eq);
    let variables = variables(p)?;
    let range = if p.at(Punct::LParen) {
        let at = p.pos;
        let (lo, hi) = numbers_in_parens(p, false)?;
        let hi = hi.unwrap_or(lo);
        if lo.trunc() > hi.trunc() {
            let message = format!("the range ({lo}, {hi}) has its high end first");
            return Err(SyntaxError::new(p.span_from(at), message));
        }
        Some((lo, hi))
    } else {
        None
    };
    Ok(NparTest::ChiSquare {
        variables,
        range,
        expected: Vec::new(),
    })
}

/// `EQUAL` or `f1, f2, ...` with `n*f` for n equal frequencies.
fn expected(p: &mut Parser) -> PResult<Vec<f64>> {
    if p.eat_keyword("EQUAL") {
        return Ok(Vec::new());
    }
    let mut values = Vec::new();
    while !at_end(p) {
        let at = p.pos;
        let value = p
            .number()
            .ok_or_else(|| p.unexpected(&["a frequency", "EQUAL"]))?;
        if p.eat(Punct::Star) {
            let frequency = p.number().ok_or_else(|| p.unexpected(&["a frequency"]))?;
            if value < 1.0 || value.fract() != 0.0 {
                let message =
                    format!("the repeat count {value} must be a whole number of at least 1");
                return Err(SyntaxError::new(p.span_from(at), message));
            }
            values.extend(std::iter::repeat_n(frequency, value as usize));
        } else {
            values.push(value);
        }
        p.eat(Punct::Comma);
    }
    if values.is_empty() {
        return Err(p.unexpected(&["a frequency", "EQUAL"]));
    }
    Ok(values)
}

fn one_sample_ks(p: &mut Parser) -> PResult<NparTest> {
    p.expect(Punct::LParen)?;
    let distribution = if p.eat_keyword("NORMAL") {
        KsDistribution::Normal
    } else if p.eat_keyword("UNIFORM") {
        KsDistribution::Uniform
    } else if p.eat_keyword("POISSON") {
        KsDistribution::Poisson
    } else if p.eat_keyword("EXPONENTIAL") {
        KsDistribution::Exponential
    } else {
        return Err(p.unexpected(&["NORMAL", "UNIFORM", "POISSON", "EXPONENTIAL"]));
    };
    let most = match distribution {
        KsDistribution::Normal | KsDistribution::Uniform => 2,
        _ => 1,
    };
    let mut parameters = Vec::new();
    loop {
        p.eat(Punct::Comma);
        let at = p.pos;
        let Some(value) = p.number() else { break };
        parameters.push(value);
        if parameters.len() > most {
            let message = format!("this distribution takes at most {most} parameters");
            return Err(SyntaxError::new(p.span_from(at), message));
        }
    }
    if parameters.len() == 1 && most == 2 {
        return Err(p.unexpected(&["a second parameter"]));
    }
    let at = p.pos;
    p.expect(Punct::RParen)?;
    let invalid = match (distribution, parameters.as_slice()) {
        (KsDistribution::Normal, [_, sd]) => {
            (*sd <= 0.0).then_some("the standard deviation must be positive")
        }
        (KsDistribution::Uniform, [min, max]) => {
            (min >= max).then_some("the minimum must be below the maximum")
        }
        (KsDistribution::Poisson | KsDistribution::Exponential, [mean]) => {
            (*mean <= 0.0).then_some("the mean must be positive")
        }
        _ => None,
    };
    if let Some(message) = invalid {
        return Err(SyntaxError::new(p.span_from(at.saturating_sub(1)), message));
    }
    if !p.eat(Punct::Eq) {
        // POISSON=varlist is the removed old form; the variables follow '='
        return Err(p.unexpected(&["'='"]));
    }
    let variables = variables(p)?;
    Ok(NparTest::KolmogorovSmirnov {
        distribution,
        parameters,
        variables,
    })
}

const PARTIAL_SUBCOMMANDS: [&str; 6] = [
    "VARIABLES",
    "SIGNIFICANCE",
    "STATISTICS",
    "FORMAT",
    "MISSING",
    "MATRIX",
];

/// `varlist [WITH varlist] BY varlist [(orders)]`
fn partial_list(p: &mut Parser) -> PResult<PartialList> {
    let rows = variables(p)?;
    let columns = if p.eat_word("WITH") {
        Some(variables(p)?)
    } else {
        None
    };
    if !p.eat_word("BY") {
        return Err(p.unexpected(&["WITH", "BY"]));
    }
    let controls = variables(p)?;
    let mut orders = Vec::new();
    if p.eat(Punct::LParen) {
        while !p.eat(Punct::RParen) {
            let at = p.pos;
            let order = p
                .number()
                .ok_or_else(|| p.unexpected(&["an order", "')'"]))?;
            if order < 1.0 || order.fract() != 0.0 || order > 100.0 {
                let message = format!("an order is a whole number from 1 to 100, got {order}");
                return Err(SyntaxError::new(p.span_from(at), message));
            }
            orders.push(order as u32);
            p.eat(Punct::Comma);
        }
        if orders.is_empty() || orders.len() > 5 {
            let message = "give one to five orders in the parentheses";
            return Err(SyntaxError::new(p.span(), message));
        }
    }
    Ok(PartialList {
        rows,
        columns,
        controls,
        orders,
    })
}

pub(crate) fn partial_corr(p: &mut Parser) -> PResult<Command> {
    let mut command = PartialCorr {
        lists: Vec::new(),
        one_tailed: false,
        zero_order: false,
        bad_corr: false,
        descriptives: false,
        format: PartialFormat::Matrix,
        pairwise: false,
        include: false,
    };
    p.eat(Punct::Slash);
    loop {
        if p.eat_keyword("VARIABLES") {
            p.eat(Punct::Eq);
            command.lists.push(partial_list(p)?);
        } else if p.eat_keyword("SIGNIFICANCE") {
            p.eat(Punct::Eq);
            const CHOICES: [(&str, &[bool]); 2] = [("TWOTAIL", &[false]), ("ONETAIL", &[true])];
            for one in keywords(p, &CHOICES, |p| p.unexpected(&names(&CHOICES)))? {
                command.one_tailed = one;
            }
        } else if p.eat_keyword("STATISTICS") {
            p.eat(Punct::Eq);
            const CHOICES: [(&str, &[u8]); 5] = [
                ("NONE", &[0]),
                ("CORR", &[1]),
                ("DESCRIPTIVES", &[2]),
                ("BADCORR", &[3]),
                ("ALL", &[1, 2, 3]),
            ];
            for choice in keywords(p, &CHOICES, |p| p.unexpected(&names(&CHOICES)))? {
                match choice {
                    0 => {
                        command.zero_order = false;
                        command.descriptives = false;
                        command.bad_corr = false;
                    }
                    1 => command.zero_order = true,
                    2 => command.descriptives = true,
                    _ => command.bad_corr = true,
                }
            }
        } else if p.eat_keyword("FORMAT") {
            p.eat(Punct::Eq);
            const CHOICES: [(&str, &[u8]); 3] =
                [("MATRIX", &[0]), ("CONDENSED", &[1]), ("SERIAL", &[2])];
            let chosen = keywords(p, &CHOICES, |p| p.unexpected(&names(&CHOICES)))?;
            // SERIAL wins over CONDENSED, as the manual says
            command.format = if chosen.contains(&2) {
                PartialFormat::Serial
            } else if chosen.contains(&1) {
                PartialFormat::Condensed
            } else {
                PartialFormat::Matrix
            };
        } else if p.eat_keyword("MISSING") {
            p.eat(Punct::Eq);
            const CHOICES: [(&str, &[u8]); 4] = [
                ("LISTWISE", &[0]),
                ("ANALYSIS", &[1]),
                ("EXCLUDE", &[2]),
                ("INCLUDE", &[3]),
            ];
            for choice in keywords(p, &CHOICES, |p| p.unexpected(&names(&CHOICES)))? {
                match choice {
                    0 => command.pairwise = false,
                    1 => command.pairwise = true,
                    2 => command.include = false,
                    _ => command.include = true,
                }
            }
        } else if !matches!(p.peek(), Some(TokenKind::Word(w)) if PARTIAL_SUBCOMMANDS.iter().any(|s| matches_keyword(w, s)))
            && matches!(p.peek(), Some(TokenKind::Word(_)))
        {
            // a variable list without the VARIABLES keyword
            command.lists.push(partial_list(p)?);
        } else {
            return Err(p.subcommand_error("PARTIAL CORR", &["MATRIX"]));
        }
        if !p.eat(Punct::Slash) {
            break;
        }
    }
    p.finish(&["'/'"])?;
    if command.lists.is_empty() {
        return Err(p.unexpected(&["VARIABLES=varlist BY controls"]));
    }
    if command.lists.len() > 25 {
        return Err(SyntaxError::new(
            p.span(),
            "PARTIAL CORR takes at most 25 variable lists",
        ));
    }
    Ok(Command::PartialCorr(command))
}
