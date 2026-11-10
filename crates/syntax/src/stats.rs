//! Parsers of the procedures that compare means and correlate: MEANS, T-TEST, ONEWAY and
//! CORRELATIONS.

use crate::ast::{Command, Means, MeansCell as M, MeansMissing, VarItem};
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
