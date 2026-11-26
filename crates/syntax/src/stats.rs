//! Parsers of the procedures that compare means and correlate: MEANS, T-TEST, ONEWAY,
//! CORRELATIONS and NONPAR CORR.

use crate::ast::{
    Command, Correlations, CorrelationsCi, CorrelationsLayout, DunnettSide, Means, MeansCell as M,
    MeansMissing, NonparCi, NonparCorr, Oneway, Posthoc, SpearmanSe, Standardizer, TTest,
    TTestGroups, TTestPairs, VarItem,
};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::{at_end, keyword_error, keywords, names};
use crate::table::matches_keyword;

/// A variable list that ends at BY, WITH, a parenthesis or the end of a subcommand.
pub(crate) fn variables(p: &mut Parser) -> PResult<Vec<VarItem>> {
    let mut items = Vec::new();
    while let Some(TokenKind::Word(w)) = p.peek() {
        if w.eq_ignore_ascii_case("BY") || w.eq_ignore_ascii_case("WITH") {
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

/// True when the word after a '/' names one of `subcommands`.
fn at_subcommand(p: &Parser, subcommands: &[&str]) -> bool {
    matches!(p.peek(), Some(TokenKind::Word(w)) if subcommands.iter().any(|s| matches_keyword(w, s)))
}

/// `(value)` or `= value` or a bare number, for CI and ALPHA settings; must lie in (0, 1).
fn level(p: &mut Parser, what: &str) -> PResult<f64> {
    let parenthesized = p.eat(Punct::LParen);
    if !parenthesized {
        p.eat(Punct::Eq);
    }
    let start = p.pos;
    let Some(value) = p.number() else {
        return Err(p.unexpected(&["a number between 0 and 1"]));
    };
    if !(value > 0.0 && value < 1.0) {
        let message = format!("{what} must lie between 0 and 1, got {value}");
        return Err(SyntaxError::new(p.span_from(start), message));
    }
    if parenthesized {
        p.expect(Punct::RParen)?;
    }
    Ok(value)
}

/// DEFAULT, and the cells when /CELLS is left out.
pub(crate) const MEANS_DEFAULT: [M; 3] = [M::Mean, M::Count, M::StdDev];

const MEANS_SUBCOMMANDS: [&str; 4] = ["TABLES", "CELLS", "STATISTICS", "MISSING"];

const MEANS_CELLS: [(&str, &[M]); 23] = [
    ("MEAN", &[M::Mean]),
    ("COUNT", &[M::Count]),
    ("STDDEV", &[M::StdDev]),
    ("MEDIAN", &[M::Median]),
    ("GMEDIAN", &[M::GroupedMedian]),
    ("SEMEAN", &[M::SeMean]),
    ("SUM", &[M::Sum]),
    ("MIN", &[M::Min]),
    ("MAX", &[M::Max]),
    ("RANGE", &[M::Range]),
    ("VARIANCE", &[M::Variance]),
    ("KURT", &[M::Kurtosis]),
    ("SEKURT", &[M::SeKurtosis]),
    ("SKEW", &[M::Skewness]),
    ("SESKEW", &[M::SeSkewness]),
    ("FIRST", &[M::First]),
    ("LAST", &[M::Last]),
    ("NPCT", &[M::Npct]),
    ("SPCT", &[M::Spct]),
    ("HARMONIC", &[M::Harmonic]),
    ("GEOMETRIC", &[M::Geometric]),
    ("DEFAULT", &MEANS_DEFAULT),
    (
        "ALL",
        &[
            M::Mean,
            M::Count,
            M::StdDev,
            M::Median,
            M::GroupedMedian,
            M::SeMean,
            M::Sum,
            M::Min,
            M::Max,
            M::Range,
            M::Variance,
            M::Kurtosis,
            M::SeKurtosis,
            M::Skewness,
            M::SeSkewness,
            M::First,
            M::Last,
            M::Npct,
            M::Spct,
            M::Harmonic,
            M::Geometric,
        ],
    ),
];

/// The keywords of MEANS /CELLS; `NPCT(var)` and `SPCT(var)` add the variable to `within`.
pub(crate) fn means_cells(p: &mut Parser, within: &mut Vec<String>) -> PResult<Vec<M>> {
    means_cells_with(p, within, &MEANS_DEFAULT)
}

/// The same with `default` as what DEFAULT stands for.
pub(crate) fn means_cells_with(
    p: &mut Parser,
    within: &mut Vec<String>,
    default: &[M],
) -> PResult<Vec<M>> {
    let mut out = Vec::new();
    while !at_end(p) {
        let found = MEANS_CELLS.iter().find(|(name, _)| p.at_keyword(name));
        let Some((name, cells)) = found else {
            return Err(p.unexpected(&names(&MEANS_CELLS)));
        };
        p.pos += 1;
        if matches!(*name, "NPCT" | "SPCT") && p.eat(Punct::LParen) {
            let variable = p.name("a control variable")?;
            p.expect(Punct::RParen)?;
            let index = within
                .iter()
                .position(|v| v.eq_ignore_ascii_case(&variable))
                .unwrap_or_else(|| {
                    within.push(variable);
                    within.len() - 1
                });
            out.push(if *name == "NPCT" {
                M::NpctIn(index)
            } else {
                M::SpctIn(index)
            });
        } else if *name == "DEFAULT" {
            out.extend_from_slice(default);
        } else {
            out.extend_from_slice(cells);
        }
        p.eat(Punct::Comma);
    }
    if out.is_empty() {
        return Err(p.unexpected(&names(&MEANS_CELLS)));
    }
    Ok(out)
}

/// `dependents [BY controls [BY ...]]`
fn means_table(p: &mut Parser) -> PResult<Vec<Vec<VarItem>>> {
    let mut dimensions = vec![variables(p)?];
    while p.eat_word("BY") {
        dimensions.push(variables(p)?);
    }
    Ok(dimensions)
}

pub(crate) fn means(p: &mut Parser) -> PResult<Command> {
    let mut command = Means {
        tables: Vec::new(),
        cells: Vec::new(),
        anova: false,
        linearity: false,
        missing: MeansMissing::Separate,
        within: Vec::new(),
    };
    let mut cells: Option<Vec<M>> = None;
    p.eat(Punct::Slash);
    let mut more = true;
    if !p.at_keyword("TABLES") {
        command.tables.push(means_table(p)?);
        more = p.eat(Punct::Slash);
    }
    while more {
        if p.eat_keyword("TABLES") {
            p.expect(Punct::Eq)?;
            command.tables.push(means_table(p)?);
        } else if p.eat_keyword("CELLS") {
            let chosen = if !p.eat(Punct::Eq) && at_end(p) {
                MEANS_DEFAULT.to_vec()
            } else {
                means_cells(p, &mut command.within)?
            };
            cells.get_or_insert_default().extend(chosen);
        } else if p.eat_keyword("STATISTICS") {
            const CHOICES: [(&str, &[u8]); 4] = [
                ("ANOVA", &[1]),
                ("LINEARITY", &[2]),
                ("ALL", &[2]),
                ("NONE", &[0]),
            ];
            let chosen = if !p.eat(Punct::Eq) && at_end(p) {
                vec![1]
            } else {
                keywords(p, &CHOICES, |p| p.unexpected(&names(&CHOICES)))?
            };
            for choice in chosen {
                command.anova = choice > 0;
                command.linearity = choice == 2;
            }
        } else if p.eat_keyword("MISSING") {
            p.eat(Punct::Eq);
            const CHOICES: [(&str, &[MeansMissing]); 3] = [
                ("TABLE", &[MeansMissing::Table]),
                ("INCLUDE", &[MeansMissing::Include]),
                ("DEPENDENT", &[MeansMissing::Dependent]),
            ];
            let chosen = keywords(p, &CHOICES, |p| p.unexpected(&names(&CHOICES)))?;
            command.missing = *chosen.last().expect("at least one keyword");
        } else if !at_subcommand(p, &MEANS_SUBCOMMANDS)
            && matches!(p.peek(), Some(TokenKind::Word(_)))
        {
            // another table list after a slash, without the TABLES keyword
            command.tables.push(means_table(p)?);
        } else {
            return Err(p.subcommand_error("MEANS", &[]));
        }
        more = p.eat(Punct::Slash);
    }
    p.finish(&["'/'", "BY"])?;
    if command.tables.is_empty() {
        return Err(p.unexpected(&["TABLES=dependents BY controls"]));
    }
    let mut chosen = cells.unwrap_or_else(|| MEANS_DEFAULT.to_vec());
    let mut seen = Vec::new();
    chosen.retain(|c| {
        let new = !seen.contains(c);
        seen.push(*c);
        new
    });
    command.cells = chosen;
    Ok(Command::Means(command))
}

const TTEST_SUBCOMMANDS: [&str; 7] = [
    "TESTVAL",
    "GROUPS",
    "VARIABLES",
    "PAIRS",
    "CRITERIA",
    "MISSING",
    "ES",
];

/// `list [WITH list [(PAIRED)]]`
pub(crate) fn pair_list(p: &mut Parser) -> PResult<TTestPairs> {
    let left = variables(p)?;
    let start = p.pos;
    let mut pairs = TTestPairs {
        left,
        right: None,
        paired: false,
    };
    if p.eat_word("WITH") {
        pairs.right = Some(variables(p)?);
    }
    if p.at(Punct::LParen) {
        p.pos += 1;
        if !p.eat_keyword("PAIRED") {
            return Err(p.unexpected(&["PAIRED"]));
        }
        p.expect(Punct::RParen)?;
        if pairs.right.is_none() {
            let message = "(PAIRED) needs a list after WITH to pair with";
            return Err(SyntaxError::new(p.span_from(start), message));
        }
        pairs.paired = true;
    }
    Ok(pairs)
}

pub(crate) fn t_test(p: &mut Parser) -> PResult<Command> {
    let mut command = TTest {
        variables: Vec::new(),
        testval: None,
        groups: None,
        pairs: Vec::new(),
        ci: 0.95,
        listwise: false,
        include: false,
        effect_sizes: true,
        standardizer: Standardizer::Sd,
    };
    let start = p.pos;
    p.eat(Punct::Slash);
    let mut last_pairs = false;
    loop {
        if p.eat_keyword("TESTVAL") {
            p.eat(Punct::Eq);
            command.testval = Some(p.number().ok_or_else(|| p.unexpected(&["a number"]))?);
            last_pairs = false;
        } else if p.eat_keyword("GROUPS") {
            p.eat(Punct::Eq);
            let name = p.name("the grouping variable")?;
            let mut groups = TTestGroups::Default;
            if p.eat(Punct::LParen) {
                let values = [p.value(), {
                    p.eat(Punct::Comma);
                    p.value()
                }];
                groups = match values {
                    [None, _] => TTestGroups::Default,
                    [Some(a), None] => TTestGroups::Cut(a),
                    [Some(a), Some(b)] => TTestGroups::Two(a, b),
                };
                p.expect(Punct::RParen)?;
            }
            command.groups = Some((name, groups));
            last_pairs = false;
        } else if p.eat_keyword("VARIABLES") {
            p.eat(Punct::Eq);
            command.variables.extend(variables(p)?);
            last_pairs = false;
        } else if p.eat_keyword("PAIRS") {
            p.eat(Punct::Eq);
            command.pairs.push(pair_list(p)?);
            last_pairs = true;
        } else if p.eat_keyword("CRITERIA") {
            p.eat(Punct::Eq);
            if !(p.eat_keyword("CIN") || p.eat_word("CI")) {
                return Err(p.unexpected(&["CI(value)"]));
            }
            command.ci = level(p, "the confidence level")?;
            last_pairs = false;
        } else if p.eat_keyword("MISSING") {
            p.eat(Punct::Eq);
            const CHOICES: [(&str, &[u8]); 4] = [
                ("ANALYSIS", &[0]),
                ("LISTWISE", &[1]),
                ("INCLUDE", &[2]),
                ("EXCLUDE", &[3]),
            ];
            for choice in keywords(p, &CHOICES, |p| p.unexpected(&names(&CHOICES)))? {
                match choice {
                    0 => command.listwise = false,
                    1 => command.listwise = true,
                    2 => command.include = true,
                    _ => command.include = false,
                }
            }
            last_pairs = false;
        } else if p.eat_keyword("ES") {
            while !at_end(p) {
                if p.eat_keyword("DISPLAY") {
                    p.expect(Punct::LParen)?;
                    command.effect_sizes = if p.eat_keyword("TRUE") {
                        true
                    } else if p.eat_keyword("FALSE") {
                        false
                    } else {
                        return Err(p.unexpected(&["TRUE", "FALSE"]));
                    };
                    p.expect(Punct::RParen)?;
                } else if p.eat_keyword("STANDARDIZER") {
                    p.expect(Punct::LParen)?;
                    command.standardizer = if p.eat_keyword("SD") {
                        Standardizer::Sd
                    } else if p.eat_keyword("CORRECTED_SD") {
                        Standardizer::CorrectedSd
                    } else if p.eat_keyword("AVERAGE") {
                        Standardizer::Average
                    } else {
                        return Err(p.unexpected(&["SD", "CORRECTED_SD", "AVERAGE"]));
                    };
                    p.expect(Punct::RParen)?;
                } else {
                    return Err(p.unexpected(&["DISPLAY(TRUE|FALSE)", "STANDARDIZER(...)"]));
                }
            }
            last_pairs = false;
        } else if last_pairs && !at_subcommand(p, &TTEST_SUBCOMMANDS) {
            // another analysis list of PAIRS after a slash
            command.pairs.push(pair_list(p)?);
        } else {
            return Err(p.subcommand_error("T-TEST", &[]));
        }
        if !p.eat(Punct::Slash) {
            break;
        }
    }
    p.finish(&["'/'"])?;
    let one_or_two = command.testval.is_some() || command.groups.is_some();
    let message = match (
        one_or_two,
        command.variables.is_empty(),
        command.pairs.is_empty(),
    ) {
        (true, true, _) => Some("TESTVAL and GROUPS need /VARIABLES=varlist"),
        (false, false, _) => Some("VARIABLES needs /TESTVAL=value or /GROUPS=var(values)"),
        (false, true, true) => Some("T-TEST needs TESTVAL, GROUPS or PAIRS"),
        _ => None,
    };
    if let Some(message) = message {
        return Err(SyntaxError::new(p.span_from(start), message));
    }
    Ok(Command::TTest(command))
}

const ONEWAY_UNSUPPORTED: [&str; 1] = ["ES"];

const POSTHOCS: [(&str, &[Posthoc]); 20] = [
    ("LSD", &[Posthoc::Lsd]),
    ("BONFERRONI", &[Posthoc::Bonferroni]),
    ("SIDAK", &[Posthoc::Sidak]),
    ("TUKEY", &[Posthoc::Tukey]),
    ("SCHEFFE", &[Posthoc::Scheffe]),
    ("GH", &[Posthoc::GamesHowell]),
    ("T2", &[Posthoc::T2]),
    ("SNK", &[Posthoc::Snk]),
    ("BTUKEY", &[Posthoc::TukeyB]),
    ("DUNCAN", &[Posthoc::Duncan]),
    ("GT2", &[Posthoc::Gt2]),
    ("GABRIEL", &[Posthoc::Gabriel]),
    ("FREGW", &[Posthoc::Fregw]),
    ("QREGW", &[Posthoc::Qregw]),
    ("T3", &[Posthoc::T3]),
    ("C", &[Posthoc::C]),
    ("DUNNETT", &[Posthoc::Dunnett(None, DunnettSide::Both)]),
    ("DUNNETTL", &[Posthoc::Dunnett(None, DunnettSide::Less)]),
    ("DUNNETTR", &[Posthoc::Dunnett(None, DunnettSide::Greater)]),
    ("WALLER", &[Posthoc::Waller(100)]),
];

/// Post hoc keywords that may not be shortened.
const FULL_POSTHOCS: [&str; 4] = ["DUNCAN", "DUNNETT", "DUNNETTL", "DUNNETTR"];

const UNSUPPORTED_POSTHOCS: [&str; 0] = [];

/// The tests of ONEWAY /RANGES. MODLSD, and LSDMOD as its alias, is Bonferroni.
const RANGE_TESTS: [(&str, Posthoc); 8] = [
    ("LSD", Posthoc::Lsd),
    ("DUNCAN", Posthoc::Duncan),
    ("SNK", Posthoc::Snk),
    ("TUKEYB", Posthoc::TukeyB),
    ("TUKEY", Posthoc::Tukey),
    ("MODLSD", Posthoc::Bonferroni),
    ("LSDMOD", Posthoc::Bonferroni),
    ("SCHEFFE", Posthoc::Scheffe),
];

/// The most `/RANGES` subcommands one ONEWAY takes.
const MOST_RANGES: usize = 10;

/// One post hoc test keyword, with the reference category of the Dunnett tests. `more` names the
/// other keywords the subcommand takes, for the error message.
pub(crate) fn posthoc_test(p: &mut Parser, place: &str, more: &[&str]) -> PResult<Posthoc> {
    let shortened = |w: &str, name: &str| {
        name.len() > 2 && !FULL_POSTHOCS.contains(&name) && matches_keyword(w, name)
    };
    let found = POSTHOCS.iter().find(|(name, _)| {
        matches!(p.peek(), Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case(name) || shortened(w, name))
    });
    let Some((_, tests)) = found else {
        let mut supported = names(&POSTHOCS);
        supported.extend(more);
        return Err(keyword_error(p, place, &UNSUPPORTED_POSTHOCS, &supported));
    };
    p.pos += 1;
    let mut test = tests[0];
    if let Posthoc::Dunnett(_, side) = test
        && p.eat(Punct::LParen)
    {
        let start = p.pos;
        let category = p.number().filter(|n| *n >= 1.0 && n.fract() == 0.0);
        let Some(category) = category else {
            let message = "the reference category is a whole number of at least 1";
            return Err(SyntaxError::new(p.span_from(start), message));
        };
        p.expect(Punct::RParen)?;
        test = Posthoc::Dunnett(Some(category as u32), side);
    }
    if test == Posthoc::Waller(100) && p.eat(Punct::LParen) {
        let start = p.pos;
        let ratio = p
            .number()
            .filter(|n| *n > 1.0 && n.fract() == 0.0 && *n <= f64::from(u32::MAX));
        let Some(ratio) = ratio else {
            let message =
                "the Type 1/Type 2 error seriousness ratio of WALLER is a whole number above 1";
            return Err(SyntaxError::new(p.span_from(start), message));
        };
        p.expect(Punct::RParen)?;
        test = Posthoc::Waller(ratio as u32);
    }
    Ok(test)
}

pub(crate) fn oneway(p: &mut Parser) -> PResult<Command> {
    p.eat(Punct::Slash);
    if p.eat_keyword("VARIABLES") {
        p.expect(Punct::Eq)?;
    }
    let dependents = variables(p)?;
    if !p.eat_word("BY") {
        return Err(p.unexpected(&["BY"]));
    }
    let factor = p.name("the factor variable")?;
    if p.at(Punct::LParen) {
        let message = "a value range after the factor is not supported; every value forms a group";
        return Err(SyntaxError::new(p.span(), message));
    }
    let mut command = Oneway {
        dependents,
        factor,
        contrasts: Vec::new(),
        posthoc: Vec::new(),
        alpha: 0.05,
        descriptives: false,
        effects: false,
        homogeneity: false,
        welch: false,
        brown_forsythe: false,
        listwise: false,
        include: false,
        ci: 0.95,
        plot_means: false,
        template: false,
        polynomial: None,
        matrix: crate::MatrixIo::default(),
    };
    // the tests of /RANGES add up, and stay when a later /POSTHOC replaces the post hoc tests
    let mut ranges: Vec<Posthoc> = Vec::new();
    let mut range_subcommands = 0;
    while p.eat(Punct::Slash) {
        if p.eat_keyword("RANGES") {
            let start = p.pos - 1;
            range_subcommands += 1;
            if range_subcommands > MOST_RANGES {
                let message = format!("ONEWAY takes at most {MOST_RANGES} RANGES subcommands");
                return Err(SyntaxError::new(p.span_from(start), message));
            }
            p.eat(Punct::Eq);
            let word = match p.peek() {
                Some(TokenKind::Word(w)) => w.to_string(),
                _ => return Err(p.unexpected(&names(&RANGE_TESTS))),
            };
            let exact = RANGE_TESTS
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(&word));
            let mut shortened = RANGE_TESTS
                .iter()
                .filter(|(n, _)| matches_keyword(&word, n));
            let found = exact.or_else(|| shortened.next().filter(|_| shortened.next().is_none()));
            let Some((_, test)) = found else {
                return Err(p.unexpected(&names(&RANGE_TESTS)));
            };
            p.pos += 1;
            if p.at(Punct::LParen) {
                // the last alpha given counts for every test
                command.alpha = level(p, "ALPHA")?;
            }
            if !ranges.contains(test) {
                ranges.push(*test);
            }
        } else if p.eat_keyword("POLYNOMIAL") {
            p.eat(Punct::Eq);
            let start = p.pos;
            let degree = p
                .number()
                .filter(|d| (1.0..=5.0).contains(d) && d.fract() == 0.0);
            let Some(degree) = degree else {
                let message = "POLYNOMIAL needs a whole number from 1 to 5";
                return Err(SyntaxError::new(p.span_from(start), message));
            };
            // only one POLYNOMIAL counts: the last
            command.polynomial = Some(degree as u8);
        } else if p.eat_keyword("POSTHOC") {
            p.eat(Punct::Eq);
            let mut chosen = Vec::new();
            while !at_end(p) {
                if p.eat_keyword("ALPHA") {
                    command.alpha = level(p, "ALPHA")?;
                    continue;
                }
                chosen.push(posthoc_test(p, "ONEWAY /POSTHOC", &["ALPHA(value)"])?);
                p.eat(Punct::Comma);
            }
            if chosen.is_empty() {
                return Err(p.unexpected(&names(&POSTHOCS)));
            }
            // only one POSTHOC counts: the last
            command.posthoc.clear();
            for test in chosen {
                if !command.posthoc.contains(&test) {
                    command.posthoc.push(test);
                }
            }
        } else if p.eat_keyword("CONTRAST") {
            p.eat(Punct::Eq);
            let mut coefficients = Vec::new();
            while !at_end(p) {
                let start = p.pos;
                let Some(value) = p.number() else {
                    return Err(p.unexpected(&["a coefficient"]));
                };
                if p.eat(Punct::Star) {
                    let Some(repeated) = p.number() else {
                        return Err(p.unexpected(&["a coefficient"]));
                    };
                    if value < 1.0 || value.fract() != 0.0 {
                        let message = "in n*c the count n must be a positive whole number";
                        return Err(SyntaxError::new(p.span_from(start), message));
                    }
                    coefficients.extend(std::iter::repeat_n(repeated, value as usize));
                } else {
                    coefficients.push(value);
                }
                p.eat(Punct::Comma);
            }
            if coefficients.is_empty() {
                return Err(p.unexpected(&["a coefficient"]));
            }
            command.contrasts.push(coefficients);
        } else if p.eat_keyword("STATISTICS") {
            p.eat(Punct::Eq);
            const CHOICES: [(&str, &[u8]); 7] = [
                ("DESCRIPTIVES", &[1]),
                ("EFFECTS", &[5]),
                ("HOMOGENEITY", &[2]),
                ("WELCH", &[3]),
                ("BROWNFORSYTHE", &[4]),
                ("ALL", &[1, 2, 3, 4, 5]),
                ("NONE", &[0]),
            ];
            let chosen = keywords(p, &CHOICES, |p| p.unexpected(&names(&CHOICES)))?;
            for choice in chosen {
                match choice {
                    1 => command.descriptives = true,
                    2 => command.homogeneity = true,
                    3 => command.welch = true,
                    4 => command.brown_forsythe = true,
                    5 => command.effects = true,
                    _ => {}
                }
            }
        } else if p.eat_keyword("MISSING") {
            p.eat(Punct::Eq);
            const CHOICES: [(&str, &[u8]); 4] = [
                ("ANALYSIS", &[0]),
                ("LISTWISE", &[1]),
                ("INCLUDE", &[2]),
                ("EXCLUDE", &[3]),
            ];
            for choice in keywords(p, &CHOICES, |p| p.unexpected(&names(&CHOICES)))? {
                match choice {
                    0 => command.listwise = false,
                    1 => command.listwise = true,
                    2 => command.include = true,
                    _ => command.include = false,
                }
            }
        } else if p.eat_keyword("CRITERIA") {
            p.eat(Punct::Eq);
            if !p.eat_keyword("CILEVEL") {
                return Err(p.unexpected(&["CILEVEL(value)"]));
            }
            command.ci = level(p, "CILEVEL")?;
        } else if p.eat_keyword("PLOT") {
            p.eat(Punct::Eq);
            if !p.eat_keyword("MEANS") {
                return Err(p.unexpected(&["MEANS"]));
            }
            command.plot_means = true;
        } else if p.eat_keyword("TEMPLATE") {
            p.eat(Punct::Eq);
            p.file("a template file")?;
            command.template = true;
        } else if p.at_keyword("MATRIX") {
            let start = p.pos;
            p.pos += 1;
            crate::matrix_io::matrix_io(p, &mut command.matrix, "ONEWAY", true, false)?;
            if command.matrix.input.is_some() && command.matrix.output.is_some() {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    "ONEWAY cannot read and write a matrix in one command: give IN or OUT, not both",
                ));
            }
        } else {
            return Err(p.subcommand_error("ONEWAY", &ONEWAY_UNSUPPORTED));
        }
    }
    p.finish(&["'/'"])?;
    for test in ranges {
        if !command.posthoc.contains(&test) {
            command.posthoc.push(test);
        }
    }
    Ok(Command::Oneway(command))
}

/// `list [WITH list]`
fn correlation_list(p: &mut Parser) -> PResult<(Vec<VarItem>, Option<Vec<VarItem>>)> {
    let rows = variables(p)?;
    let columns = if p.eat_word("WITH") {
        Some(variables(p)?)
    } else {
        None
    };
    Ok((rows, columns))
}

/// `(value)` or `= value` in percent, between 0 and 100 (CILEVEL of the CI subcommands).
fn percent(p: &mut Parser, what: &str) -> PResult<f64> {
    let parenthesized = p.eat(Punct::LParen);
    if !parenthesized {
        p.eat(Punct::Eq);
    }
    let start = p.pos;
    let Some(value) = p.number() else {
        return Err(p.unexpected(&["a percent between 0 and 100"]));
    };
    if !(value > 0.0 && value < 100.0) {
        return Err(SyntaxError::new(
            p.span_from(start),
            format!("{what} must lie between 0 and 100, got {value}"),
        ));
    }
    if parenthesized {
        p.expect(Punct::RParen)?;
    }
    Ok(value)
}

/// MATRIX of CORRELATIONS and NONPAR CORR, which only write: `OUT(file | dataset | *)`.
fn matrix_out_only(p: &mut Parser, io: &mut crate::matrix_io::MatrixIo, who: &str) -> PResult<()> {
    let start = p.pos;
    crate::matrix_io::matrix_io(p, io, who, false, false)?;
    if io.input.is_some() {
        return Err(SyntaxError::new(
            p.span_from(start),
            format!("{who} /MATRIX writes a matrix file: give OUT(...), not IN"),
        ));
    }
    Ok(())
}

const CORRELATIONS_SUBCOMMANDS: [&str; 6] = [
    "VARIABLES",
    "MISSING",
    "PRINT",
    "STATISTICS",
    "MATRIX",
    "CI",
];

pub(crate) fn correlations(p: &mut Parser) -> PResult<Command> {
    let mut command = Correlations {
        lists: Vec::new(),
        one_tailed: false,
        flag: false,
        layout: CorrelationsLayout::Full,
        descriptives: false,
        xprod: false,
        listwise: false,
        include: false,
        matrix: crate::matrix_io::MatrixIo::default(),
        ci: None,
    };
    p.eat(Punct::Slash);
    let mut more = true;
    if !p.at_keyword("VARIABLES") {
        command.lists.push(correlation_list(p)?);
        more = p.eat(Punct::Slash);
    }
    while more {
        if p.eat_keyword("VARIABLES") {
            p.expect(Punct::Eq)?;
            command.lists.push(correlation_list(p)?);
        } else if p.at_keyword("MATRIX") {
            p.pos += 1;
            matrix_out_only(p, &mut command.matrix, "CORRELATIONS")?;
        } else if p.eat_keyword("CI") {
            let mut ci = CorrelationsCi {
                bias: false,
                level: 95.0,
            };
            while !at_end(p) {
                if p.eat_keyword("METHOD") {
                    p.eat(Punct::Eq);
                    if p.eat_keyword("EXACT") {
                        return Err(SyntaxError::new(
                            p.span(),
                            "CI METHOD=EXACT is not supported yet; the manual itself says it is not available in its release. FISHER is",
                        ));
                    }
                    if !p.eat_keyword("FISHER") {
                        return Err(p.unexpected(&["FISHER"]));
                    }
                } else if p.eat_keyword("BIAS") {
                    p.eat(Punct::Eq);
                    ci.bias = if p.eat_keyword("TRUE") {
                        true
                    } else if p.eat_keyword("FALSE") {
                        false
                    } else {
                        return Err(p.unexpected(&["TRUE", "FALSE"]));
                    };
                } else if p.eat_keyword("CILEVEL") {
                    ci.level = percent(p, "CILEVEL")?;
                } else {
                    return Err(p.unexpected(&["METHOD", "BIAS", "CILEVEL"]));
                }
                p.eat(Punct::Comma);
            }
            command.ci = Some(ci);
        } else if p.eat_keyword("MISSING") {
            p.eat(Punct::Eq);
            const CHOICES: [(&str, &[u8]); 4] = [
                ("PAIRWISE", &[0]),
                ("LISTWISE", &[1]),
                ("INCLUDE", &[2]),
                ("EXCLUDE", &[3]),
            ];
            for choice in keywords(p, &CHOICES, |p| p.unexpected(&names(&CHOICES)))? {
                match choice {
                    0 => command.listwise = false,
                    1 => command.listwise = true,
                    2 => command.include = true,
                    _ => command.include = false,
                }
            }
        } else if p.eat_keyword("PRINT") {
            p.eat(Punct::Eq);
            const CHOICES: [(&str, &[u8]); 7] = [
                ("TWOTAIL", &[0]),
                ("ONETAIL", &[1]),
                ("SIG", &[2]),
                ("NOSIG", &[3]),
                ("FULL", &[4]),
                ("LOWER", &[5]),
                ("LNODIAG", &[6]),
            ];
            let chosen = keywords(p, &CHOICES, |p| {
                keyword_error(p, "CORRELATIONS /PRINT", &["NOMATRIX"], &names(&CHOICES))
            })?;
            for choice in chosen {
                match choice {
                    0 => command.one_tailed = false,
                    1 => command.one_tailed = true,
                    2 => command.flag = false,
                    3 => command.flag = true,
                    4 => command.layout = CorrelationsLayout::Full,
                    5 => command.layout = CorrelationsLayout::Lower,
                    _ => command.layout = CorrelationsLayout::LowerNoDiagonal,
                }
            }
        } else if p.eat_keyword("STATISTICS") {
            p.eat(Punct::Eq);
            const CHOICES: [(&str, &[u8]); 3] =
                [("DESCRIPTIVES", &[1]), ("XPROD", &[2]), ("ALL", &[1, 2])];
            for choice in keywords(p, &CHOICES, |p| p.unexpected(&names(&CHOICES)))? {
                match choice {
                    1 => command.descriptives = true,
                    _ => command.xprod = true,
                }
            }
        } else if !at_subcommand(p, &CORRELATIONS_SUBCOMMANDS)
            && matches!(p.peek(), Some(TokenKind::Word(_)))
        {
            // another variable list after a slash, without the VARIABLES keyword
            command.lists.push(correlation_list(p)?);
        } else {
            return Err(p.subcommand_error("CORRELATIONS", &[]));
        }
        more = p.eat(Punct::Slash);
    }
    p.finish(&["'/'", "WITH"])?;
    if command.lists.is_empty() {
        return Err(p.unexpected(&["VARIABLES=varlist"]));
    }
    Ok(Command::Correlations(command))
}

const NONPAR_CORR_SUBCOMMANDS: [&str; 6] =
    ["VARIABLES", "PRINT", "MISSING", "SAMPLE", "MATRIX", "CI"];

pub(crate) fn nonpar_corr(p: &mut Parser) -> PResult<Command> {
    let mut command = NonparCorr {
        lists: Vec::new(),
        spearman: true,
        kendall: false,
        one_tailed: false,
        flag: false,
        layout: CorrelationsLayout::Full,
        listwise: false,
        include: false,
        matrix: crate::matrix_io::MatrixIo::default(),
        ci: None,
    };
    p.eat(Punct::Slash);
    let mut more = true;
    if !p.at_keyword("VARIABLES") {
        command.lists.push(correlation_list(p)?);
        more = p.eat(Punct::Slash);
    }
    while more {
        if p.eat_keyword("VARIABLES") {
            p.expect(Punct::Eq)?;
            command.lists.push(correlation_list(p)?);
        } else if p.at_keyword("MATRIX") {
            p.pos += 1;
            matrix_out_only(p, &mut command.matrix, "NONPAR CORR")?;
        } else if p.eat_keyword("CI") {
            let mut ci = NonparCi {
                method: SpearmanSe::Fhp,
                level: 95.0,
            };
            while !at_end(p) {
                if p.eat_keyword("METHOD") {
                    let parenthesized = p.eat(Punct::LParen);
                    if !parenthesized {
                        p.eat(Punct::Eq);
                    }
                    ci.method = if p.eat_keyword("FHP") {
                        SpearmanSe::Fhp
                    } else if p.eat_keyword("CC") {
                        SpearmanSe::Cc
                    } else if p.eat_keyword("BW") {
                        SpearmanSe::Bw
                    } else {
                        return Err(p.unexpected(&["FHP", "CC", "BW"]));
                    };
                    if parenthesized {
                        p.expect(Punct::RParen)?;
                    }
                } else if p.eat_keyword("CILEVEL") {
                    ci.level = percent(p, "CILEVEL")?;
                } else {
                    return Err(p.unexpected(&["METHOD", "CILEVEL"]));
                }
                p.eat(Punct::Comma);
            }
            command.ci = Some(ci);
        } else if p.eat_keyword("MISSING") {
            p.eat(Punct::Eq);
            const CHOICES: [(&str, &[u8]); 3] =
                [("PAIRWISE", &[0]), ("LISTWISE", &[1]), ("INCLUDE", &[2])];
            for choice in keywords(p, &CHOICES, |p| p.unexpected(&names(&CHOICES)))? {
                match choice {
                    0 => command.listwise = false,
                    1 => command.listwise = true,
                    _ => command.include = true,
                }
            }
        } else if p.eat_keyword("PRINT") {
            p.eat(Punct::Eq);
            const CHOICES: [(&str, &[u8]); 11] = [
                ("TWOTAIL", &[0]),
                ("ONETAIL", &[1]),
                ("SIG", &[2]),
                ("NOSIG", &[3]),
                ("SPEARMAN", &[4]),
                ("KENDALL", &[5]),
                ("BOTH", &[6]),
                ("FULL", &[7]),
                ("LOWER", &[8]),
                ("LNODIAG", &[9]),
                // the table is suppressed only together with /CI; without it FULL applies
                ("NOMATRIX", &[7]),
            ];
            for choice in keywords(p, &CHOICES, |p| p.unexpected(&names(&CHOICES)))? {
                match choice {
                    0 => command.one_tailed = false,
                    1 => command.one_tailed = true,
                    2 => command.flag = false,
                    3 => command.flag = true,
                    4 => (command.spearman, command.kendall) = (true, false),
                    5 => (command.spearman, command.kendall) = (false, true),
                    6 => (command.spearman, command.kendall) = (true, true),
                    7 => command.layout = CorrelationsLayout::Full,
                    8 => command.layout = CorrelationsLayout::Lower,
                    _ => command.layout = CorrelationsLayout::LowerNoDiagonal,
                }
            }
        } else if p.eat_keyword("SAMPLE") {
            // sampling applies only when the cases do not fit in memory, which they always do
        } else if !at_subcommand(p, &NONPAR_CORR_SUBCOMMANDS)
            && matches!(p.peek(), Some(TokenKind::Word(_)))
        {
            // another variable list after a slash, without the VARIABLES keyword
            command.lists.push(correlation_list(p)?);
        } else {
            return Err(p.subcommand_error("NONPAR CORR", &[]));
        }
        more = p.eat(Punct::Slash);
    }
    p.finish(&["'/'", "WITH"])?;
    if command.lists.is_empty() {
        return Err(p.unexpected(&["VARIABLES=varlist"]));
    }
    Ok(Command::NonparCorr(command))
}
