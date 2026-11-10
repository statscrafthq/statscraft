//! Parsers of the statistical procedures DESCRIPTIVES and CROSSTABS.

use crate::ast::{
    Command, Crosstabs, CrosstabsCell, DescriptiveStatistic as Stat, Descriptives, VarItem,
};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::table::matches_keyword;

const DESCRIPTIVES_SUBCOMMANDS: [&str; 2] = ["SORT", "FORMAT"];

const STATISTICS: [(&str, &[Stat]); 14] = [
    ("MEAN", &[Stat::Mean]),
    ("SEMEAN", &[Stat::SeMean]),
    ("STDDEV", &[Stat::StdDev]),
    ("VARIANCE", &[Stat::Variance]),
    ("KURTOSIS", &[Stat::Kurtosis]),
    ("SEKURTOSIS", &[Stat::Kurtosis]),
    ("SKEWNESS", &[Stat::Skewness]),
    ("SESKEWNESS", &[Stat::Skewness]),
    ("RANGE", &[Stat::Range]),
    ("MINIMUM", &[Stat::Min]),
    ("MAXIMUM", &[Stat::Max]),
    ("SUM", &[Stat::Sum]),
    ("DEFAULT", &[Stat::Mean, Stat::StdDev, Stat::Min, Stat::Max]),
    (
        "ALL",
        &[
            Stat::Mean,
            Stat::SeMean,
            Stat::StdDev,
            Stat::Variance,
            Stat::Kurtosis,
            Stat::Skewness,
            Stat::Range,
            Stat::Min,
            Stat::Max,
            Stat::Sum,
        ],
    ),
];

const CROSSTABS_SUBCOMMANDS: [&str; 6] =
    ["VARIABLES", "WRITE", "COUNT", "METHOD", "BARCHART", "EXACT"];

const CELLS: [(&str, &[CrosstabsCell]); 10] = [
    ("COUNT", &[CrosstabsCell::Count]),
    ("ROW", &[CrosstabsCell::Row]),
    ("COLUMN", &[CrosstabsCell::Column]),
    ("TOTAL", &[CrosstabsCell::Total]),
    ("EXPECTED", &[CrosstabsCell::Expected]),
    ("RESID", &[CrosstabsCell::Resid]),
    ("SRESID", &[CrosstabsCell::Sresid]),
    ("ASRESID", &[CrosstabsCell::Asresid]),
    (
        "ALL",
        &[
            CrosstabsCell::Count,
            CrosstabsCell::Row,
            CrosstabsCell::Column,
            CrosstabsCell::Total,
            CrosstabsCell::Expected,
            CrosstabsCell::Resid,
            CrosstabsCell::Sresid,
            CrosstabsCell::Asresid,
        ],
    ),
    ("NONE", &[]),
];

const UNSUPPORTED_CELLS: [&str; 3] = ["BPROP", "PROP", "SUMMARY"];

const UNSUPPORTED_STATISTICS: [&str; 14] = [
    "PHI", "CC", "LAMBDA", "UC", "BTAU", "CTAU", "RISK", "GAMMA", "D", "KAPPA", "ETA", "CORR",
    "CMH", "ALL",
];

const UNSUPPORTED_FORMATS: [&str; 8] = [
    "TABLES", "NOTABLES", "PIVOT", "NOPIVOT", "BOX", "NOBOX", "INDEX", "NOINDEX",
];

/// Error at a keyword of a subcommand: "not supported yet" for one of `unsupported`, otherwise
/// "unexpected" with the supported keywords as the expected ones.
pub(crate) fn keyword_error(
    p: &Parser,
    place: &str,
    unsupported: &[&str],
    supported: &[&str],
) -> SyntaxError {
    let known = match p.peek() {
        Some(TokenKind::Word(w)) => unsupported.iter().find(|k| matches_keyword(w, k)),
        _ => None,
    };
    match known {
        Some(name) => {
            let mut error =
                SyntaxError::new(p.span(), format!("{name} in {place} is not supported yet"));
            error.expected = supported.iter().map(|s| s.to_string()).collect();
            error
        }
        None => p.unexpected(supported),
    }
}

/// Keywords of a subcommand up to the next '/' or the end, each mapped to its values. At least
/// one keyword is needed.
pub(crate) fn keywords<T: Copy>(
    p: &mut Parser,
    table: &[(&str, &[T])],
    on_unknown: impl Fn(&Parser) -> SyntaxError,
) -> PResult<Vec<T>> {
    let mut out = Vec::new();
    let mut any = false;
    while !at_end(p) {
        let found = table.iter().find(|(name, _)| p.at_keyword(name));
        let Some((_, values)) = found else {
            return Err(on_unknown(p));
        };
        p.pos += 1;
        out.extend_from_slice(values);
        any = true;
        p.eat(Punct::Comma);
    }
    if !any {
        return Err(on_unknown(p));
    }
    Ok(out)
}

/// True at the end of the command or of a subcommand.
pub(crate) fn at_end(p: &Parser) -> bool {
    matches!(p.peek(), None | Some(TokenKind::Punct(Punct::Slash)))
}

pub(crate) fn names<T>(table: &[(&'static str, T)]) -> Vec<&'static str> {
    table.iter().map(|(name, _)| *name).collect()
}

/// A variable list in which a single variable may be followed by `(name)` for its z-score.
fn z_list(p: &mut Parser) -> PResult<Vec<(VarItem, Option<String>)>> {
    let mut items = Vec::new();
    // BY separates the dimensions of CROSSTABS
    while matches!(p.peek(), Some(TokenKind::Word(w)) if !w.eq_ignore_ascii_case("BY")) {
        let start = p.pos;
        let item = if p.eat_word("ALL") {
            VarItem::All
        } else {
            let first = p.name("a variable name")?;
            if p.eat_word("TO") {
                VarItem::Range(first, p.name("a variable name")?)
            } else {
                VarItem::Name(first)
            }
        };
        let z = if p.eat(Punct::LParen) {
            let name = p.name("a name for the z-score variable")?;
            p.expect(Punct::RParen)?;
            if !matches!(item, VarItem::Name(_)) {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    "a z-score name in parentheses can follow a single variable only",
                ));
            }
            Some(name)
        } else {
            None
        };
        items.push((item, z));
        p.eat(Punct::Comma);
    }
    if items.is_empty() {
        return Err(p.unexpected(&["a variable name"]));
    }
    Ok(items)
}

pub(crate) fn descriptives(p: &mut Parser) -> PResult<Command> {
    let mut command = Descriptives {
        variables: Vec::new(),
        statistics: Vec::new(),
        listwise: false,
        include: false,
        save: false,
    };
    let mut statistics: Option<Vec<Stat>> = None;
    p.eat(Punct::Slash);
    // a leading word without '=' starts the variable list, as in `DESCRIPTIVES age income`
    let bare = p.peek_at(1) != Some(&TokenKind::Punct(Punct::Eq)) && !p.at_keyword("SAVE");
    let mut more = true;
    if bare {
        command.variables = z_list(p)?;
        more = p.eat(Punct::Slash);
    }
    while more {
        if p.eat_keyword("VARIABLES") {
            p.expect(Punct::Eq)?;
            command.variables.extend(z_list(p)?);
        } else if p.eat_keyword("STATISTICS") {
            p.eat(Punct::Eq);
            let chosen = keywords(p, &STATISTICS, |p| p.unexpected(&names(&STATISTICS)))?;
            statistics.get_or_insert_default().extend(chosen);
        } else if p.eat_keyword("MISSING") {
            p.eat(Punct::Eq);
            const MISSING: [(&str, &[u8]); 3] =
                [("VARIABLE", &[0]), ("LISTWISE", &[1]), ("INCLUDE", &[2])];
            for choice in keywords(p, &MISSING, |p| p.unexpected(&names(&MISSING)))? {
                match choice {
                    0 => command.listwise = false,
                    1 => command.listwise = true,
                    _ => command.include = true,
                }
            }
        } else if p.eat_keyword("SAVE") {
            command.save = true;
        } else {
            return Err(p.subcommand_error("DESCRIPTIVES", &DESCRIPTIVES_SUBCOMMANDS));
        }
        more = p.eat(Punct::Slash);
    }
    p.finish(&["'/'"])?;
    if command.variables.is_empty() {
        return Err(p.unexpected(&["VARIABLES=varlist"]));
    }
    let mut statistics =
        statistics.unwrap_or_else(|| vec![Stat::Mean, Stat::StdDev, Stat::Min, Stat::Max]);
    statistics.sort();
    statistics.dedup();
    command.statistics = statistics;
    Ok(Command::Descriptives(command))
}

/// One variable list of CROSSTABS, ending at BY.
fn dimension(p: &mut Parser) -> PResult<Vec<VarItem>> {
    let start = p.pos;
    let items = z_list(p)?;
    if items.iter().any(|(_, z)| z.is_some()) {
        let message = "a name in parentheses is only for DESCRIPTIVES";
        return Err(SyntaxError::new(p.span_from(start), message));
    }
    Ok(items.into_iter().map(|(item, _)| item).collect())
}

/// `varlist BY varlist [BY varlist ...]`
fn crosstabs_table(p: &mut Parser) -> PResult<Vec<Vec<VarItem>>> {
    let mut dimensions = vec![dimension(p)?];
    while p.eat_word("BY") {
        dimensions.push(dimension(p)?);
    }
    if dimensions.len() < 2 {
        return Err(p.unexpected(&["BY"]));
    }
    Ok(dimensions)
}

pub(crate) fn crosstabs(p: &mut Parser) -> PResult<Command> {
    let mut command = Crosstabs {
        tables: Vec::new(),
        cells: Vec::new(),
        chisq: false,
        include: false,
        descending: false,
    };
    let mut cells: Option<Vec<CrosstabsCell>> = None;
    p.eat(Punct::Slash);
    let mut more = true;
    if p.peek_at(1) != Some(&TokenKind::Punct(Punct::Eq)) {
        command.tables.push(crosstabs_table(p)?);
        more = p.eat(Punct::Slash);
    }
    while more {
        if p.eat_keyword("TABLES") {
            p.expect(Punct::Eq)?;
            command.tables.push(crosstabs_table(p)?);
        } else if p.eat_keyword("CELLS") {
            // without keywords CELLS shows the counts and all three percentages
            let chosen = if !p.eat(Punct::Eq) && at_end(p) {
                vec![
                    CrosstabsCell::Count,
                    CrosstabsCell::Row,
                    CrosstabsCell::Column,
                    CrosstabsCell::Total,
                ]
            } else {
                keywords(p, &CELLS, |p| {
                    keyword_error(p, "CROSSTABS /CELLS", &UNSUPPORTED_CELLS, &names(&CELLS))
                })?
            };
            cells.get_or_insert_default().extend(chosen);
        } else if p.eat_keyword("STATISTICS") {
            const CHOICES: [(&str, &[bool]); 2] = [("CHISQ", &[true]), ("NONE", &[false])];
            // without keywords STATISTICS is the chi-square test
            command.chisq = if !p.eat(Punct::Eq) && at_end(p) {
                true
            } else {
                let chosen = keywords(p, &CHOICES, |p| {
                    keyword_error(
                        p,
                        "CROSSTABS /STATISTICS",
                        &UNSUPPORTED_STATISTICS,
                        &names(&CHOICES),
                    )
                })?;
                chosen.contains(&true)
            };
        } else if p.eat_keyword("MISSING") {
            p.eat(Punct::Eq);
            const CHOICES: [(&str, &[bool]); 2] = [("TABLE", &[false]), ("INCLUDE", &[true])];
            let chosen = keywords(p, &CHOICES, |p| {
                keyword_error(p, "CROSSTABS /MISSING", &["REPORT"], &names(&CHOICES))
            })?;
            command.include = chosen.last() == Some(&true);
        } else if p.eat_keyword("FORMAT") {
            p.eat(Punct::Eq);
            const CHOICES: [(&str, &[bool]); 2] = [("AVALUE", &[false]), ("DVALUE", &[true])];
            let chosen = keywords(p, &CHOICES, |p| {
                keyword_error(
                    p,
                    "CROSSTABS /FORMAT",
                    &UNSUPPORTED_FORMATS,
                    &names(&CHOICES),
                )
            })?;
            command.descending = chosen.last() == Some(&true);
        } else {
            return Err(p.subcommand_error("CROSSTABS", &CROSSTABS_SUBCOMMANDS));
        }
        more = p.eat(Punct::Slash);
    }
    p.finish(&["'/'"])?;
    if command.tables.is_empty() {
        return Err(p.unexpected(&["TABLES=rows BY columns"]));
    }
    let mut cells = cells.unwrap_or_else(|| vec![CrosstabsCell::Count]);
    cells.sort();
    cells.dedup();
    command.cells = cells;
    Ok(Command::Crosstabs(command))
}
