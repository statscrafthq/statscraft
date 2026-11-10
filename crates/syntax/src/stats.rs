//! Parsers of the procedures that compare means and correlate: MEANS, T-TEST, ONEWAY,
//! CORRELATIONS and NONPAR CORR.

use crate::ast::{
    Command, Correlations, CorrelationsLayout, Means, MeansCell as M, MeansMissing, NonparCorr,
    Oneway, Posthoc, Standardizer, TTest, TTestGroups, TTestPairs, VarItem,
};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::{at_end, keyword_error, keywords, names};
use crate::table::matches_keyword;

/// A variable list that ends at BY, WITH, a parenthesis or the end of a subcommand.
fn variables(p: &mut Parser) -> PResult<Vec<VarItem>> {
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

const MEANS_SUBCOMMANDS: [&str; 4] = ["TABLES", "CELLS", "STATISTICS", "MISSING"];

const MEANS_CELLS: [(&str, &[M]); 22] = [
    ("MEAN", &[M::Mean]),
    ("COUNT", &[M::Count]),
    ("STDDEV", &[M::StdDev]),
    ("MEDIAN", &[M::Median]),
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
    ("DEFAULT", &[M::Mean, M::Count, M::StdDev]),
    (
        "ALL",
        &[
            M::Mean,
            M::Count,
            M::StdDev,
            M::Median,
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
                vec![M::Mean, M::Count, M::StdDev]
            } else {
                keywords(p, &MEANS_CELLS, |p| {
                    if p.at(Punct::LParen) {
                        let message =
                            "NPCT(var) and SPCT(var) in MEANS /CELLS are not supported yet";
                        SyntaxError::new(p.span(), message)
                    } else {
                        keyword_error(p, "MEANS /CELLS", &["GMEDIAN"], &names(&MEANS_CELLS))
                    }
                })?
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
    let mut chosen = cells.unwrap_or_else(|| vec![M::Mean, M::Count, M::StdDev]);
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
fn pair_list(p: &mut Parser) -> PResult<TTestPairs> {
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

const ONEWAY_UNSUPPORTED: [&str; 6] = ["POLYNOMIAL", "RANGES", "PLOT", "MATRIX", "TEMPLATE", "ES"];

const POSTHOCS: [(&str, &[Posthoc]); 7] = [
    ("LSD", &[Posthoc::Lsd]),
    ("BONFERRONI", &[Posthoc::Bonferroni]),
    ("SIDAK", &[Posthoc::Sidak]),
    ("TUKEY", &[Posthoc::Tukey]),
    ("SCHEFFE", &[Posthoc::Scheffe]),
    ("GH", &[Posthoc::GamesHowell]),
    ("T2", &[Posthoc::T2]),
];

const UNSUPPORTED_POSTHOCS: [&str; 13] = [
    "SNK", "BTUKEY", "DUNCAN", "DUNNETT", "DUNNETTL", "DUNNETTR", "GT2", "GABRIEL", "FREGW",
    "QREGW", "T3", "C", "WALLER",
];

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
        homogeneity: false,
        welch: false,
        brown_forsythe: false,
        listwise: false,
        include: false,
        ci: 0.95,
    };
    while p.eat(Punct::Slash) {
        if p.eat_keyword("POSTHOC") {
            p.eat(Punct::Eq);
            let mut chosen = Vec::new();
            while !at_end(p) {
                if p.eat_keyword("ALPHA") {
                    command.alpha = level(p, "ALPHA")?;
                    continue;
                }
                let found = POSTHOCS.iter().find(|(name, _)| {
                    matches!(p.peek(), Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case(name) || (name.len() > 2 && matches_keyword(w, name)))
                });
                let Some((_, tests)) = found else {
                    let mut supported = names(&POSTHOCS);
                    supported.push("ALPHA(value)");
                    return Err(keyword_error(
                        p,
                        "ONEWAY /POSTHOC",
                        &UNSUPPORTED_POSTHOCS,
                        &supported,
                    ));
                };
                p.pos += 1;
                chosen.extend_from_slice(tests);
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
            const CHOICES: [(&str, &[u8]); 6] = [
                ("DESCRIPTIVES", &[1]),
                ("HOMOGENEITY", &[2]),
                ("WELCH", &[3]),
                ("BROWNFORSYTHE", &[4]),
                ("ALL", &[1, 2, 3, 4]),
                ("NONE", &[0]),
            ];
            let chosen = keywords(p, &CHOICES, |p| {
                keyword_error(p, "ONEWAY /STATISTICS", &["EFFECTS"], &names(&CHOICES))
            })?;
            for choice in chosen {
                match choice {
                    1 => command.descriptives = true,
                    2 => command.homogeneity = true,
                    3 => command.welch = true,
                    4 => command.brown_forsythe = true,
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
        } else {
            return Err(p.subcommand_error("ONEWAY", &ONEWAY_UNSUPPORTED));
        }
    }
    p.finish(&["'/'"])?;
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
            return Err(p.subcommand_error("CORRELATIONS", &["MATRIX", "CI"]));
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
            return Err(p.subcommand_error("NONPAR CORR", &["MATRIX", "CI"]));
        }
        more = p.eat(Punct::Slash);
    }
    p.finish(&["'/'", "WITH"])?;
    if command.lists.is_empty() {
        return Err(p.unexpected(&["VARIABLES=varlist"]));
    }
    Ok(Command::NonparCorr(command))
}
