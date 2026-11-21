//! Parsers of the statistical procedures DESCRIPTIVES and CROSSTABS.

use crate::ast::{
    Command, Crosstabs, CrosstabsCell, CrosstabsMethod, CrosstabsWrite, DescriptiveSort,
    DescriptiveStatistic as Stat, Descriptives, VarItem, WeightCount,
};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::table::matches_keyword;

const DESCRIPTIVES_SUBCOMMANDS: [&str; 1] = ["FORMAT"];

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

const CROSSTABS_SUBCOMMANDS: [&str; 1] = ["EXACT"];

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

/// The words of /FORMAT that shaped the old text listing; they are read and change nothing.
const LAYOUT_FORMATS: [&str; 6] = ["PIVOT", "NOPIVOT", "BOX", "NOBOX", "INDEX", "NOINDEX"];

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

/// `SORT` of DESCRIPTIVES: an optional key (the mean when left out) and an optional (A) or (D).
fn descriptive_sort(p: &mut Parser) -> PResult<DescriptiveSort> {
    const KEYS: [(&str, Option<Stat>); 11] = [
        ("MEAN", Some(Stat::Mean)),
        ("SEMEAN", Some(Stat::SeMean)),
        ("STDDEV", Some(Stat::StdDev)),
        ("VARIANCE", Some(Stat::Variance)),
        ("KURTOSIS", Some(Stat::Kurtosis)),
        ("SKEWNESS", Some(Stat::Skewness)),
        ("RANGE", Some(Stat::Range)),
        ("MIN", Some(Stat::Min)),
        ("MAX", Some(Stat::Max)),
        ("SUM", Some(Stat::Sum)),
        ("NAME", None),
    ];
    let mut by = Some(Stat::Mean);
    if let Some((_, key)) = KEYS.iter().find(|(name, _)| p.at_keyword(name)) {
        by = *key;
        p.pos += 1;
    } else if !at_end(p) && !p.at(Punct::LParen) {
        return Err(p.unexpected(&names(&KEYS)));
    }
    let mut descending = false;
    if p.eat(Punct::LParen) {
        descending = if p.eat_word("D") {
            true
        } else if p.eat_word("A") {
            false
        } else {
            return Err(p.unexpected(&["A", "D"]));
        };
        p.expect(Punct::RParen)?;
    }
    Ok(DescriptiveSort { by, descending })
}

pub(crate) fn descriptives(p: &mut Parser) -> PResult<Command> {
    let mut command = Descriptives {
        variables: Vec::new(),
        statistics: Vec::new(),
        listwise: false,
        include: false,
        save: false,
        sort: None,
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
        } else if p.eat_keyword("SORT") {
            p.eat(Punct::Eq);
            command.sort = Some(descriptive_sort(p)?);
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

/// `MC [CIN(level)] [SAMPLES(n)]` or `EXACT [TIMER(minutes)]`, each setting at most once.
fn crosstabs_method(p: &mut Parser) -> PResult<CrosstabsMethod> {
    let exact = if p.eat_word("MC") {
        false
    } else if p.eat_keyword("EXACT") {
        true
    } else {
        return Err(p.unexpected(&["MC", "EXACT"]));
    };
    let (mut level, mut samples, mut minutes) = (99.0, 10_000.0, 5.0);
    let mut seen: Vec<&str> = Vec::new();
    while !at_end(p) {
        let at = p.pos;
        let names: &[&str] = if exact {
            &["TIMER"]
        } else {
            &["CIN", "SAMPLES"]
        };
        let Some(&name) = names.iter().find(|n| p.at_keyword(n)) else {
            let other = ["MC", "EXACT", "CIN", "SAMPLES", "TIMER"];
            if other.iter().any(|n| p.at_keyword(n)) {
                let message = match exact {
                    true => "EXACT takes only TIMER; MC and EXACT exclude each other",
                    false => "MC takes only CIN and SAMPLES; MC and EXACT exclude each other",
                };
                return Err(SyntaxError::new(p.span_from(at), message));
            }
            return Err(p.unexpected(names));
        };
        p.pos += 1;
        if seen.contains(&name) {
            let message = format!("{name} is given twice");
            return Err(SyntaxError::new(p.span_from(at), message));
        }
        seen.push(name);
        let value = crate::npar::setting(p)?;
        let (ok, rule) = match name {
            "CIN" => ((0.01..=99.9).contains(&value), "a level from 0.01 to 99.9"),
            "SAMPLES" => (
                value.fract() == 0.0 && (1.0..=1e9).contains(&value),
                "a whole number from 1 to 1000000000",
            ),
            _ => (
                value.fract() == 0.0 && value >= 0.0,
                "a whole number of minutes, 0 for no limit",
            ),
        };
        if !ok {
            let message = format!("{name} needs {rule}, got {value}");
            return Err(SyntaxError::new(p.span_from(at), message));
        }
        match name {
            "CIN" => level = value,
            "SAMPLES" => samples = value,
            _ => minutes = value,
        }
        p.eat(Punct::Comma);
    }
    Ok(match exact {
        true => CrosstabsMethod::Exact { minutes },
        false => CrosstabsMethod::MonteCarlo {
            level,
            samples: samples as u64,
        },
    })
}

pub(crate) fn crosstabs(p: &mut Parser) -> PResult<Command> {
    let mut command = Crosstabs {
        tables: Vec::new(),
        cells: Vec::new(),
        chisq: false,
        measures: Vec::new(),
        cmh_odds: 1.0,
        include: false,
        descending: false,
        count: WeightCount::Asis,
        truncate: false,
        barchart: false,
        ranges: Vec::new(),
        report: false,
        method: None,
        write: None,
        layout_words: Vec::new(),
    };
    let mut cells: Option<Vec<CrosstabsCell>> = None;
    let mut no_tables = false;
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
        } else if p.eat_keyword("VARIABLES") {
            if !command.tables.is_empty() {
                let message = "VARIABLES must come before TABLES";
                return Err(SyntaxError::new(p.span_from(p.pos - 1), message));
            }
            p.eat(Punct::Eq);
            while !at_end(p) {
                let list = p.var_list()?;
                if list.is_empty() {
                    return Err(p.unexpected(&["a variable name"]));
                }
                p.expect(Punct::LParen)?;
                let start = p.pos;
                // the bounds are whole numbers; a fraction is cut off
                let low = p
                    .number()
                    .ok_or_else(|| p.unexpected(&["the lowest value"]))?;
                p.eat(Punct::Comma);
                let high = p
                    .number()
                    .ok_or_else(|| p.unexpected(&["the highest value"]))?;
                let (low, high) = (low.trunc(), high.trunc());
                if low > high {
                    let message = format!("the lowest value {low} is above the highest {high}");
                    return Err(SyntaxError::new(p.span_from(start), message));
                }
                p.expect(Punct::RParen)?;
                command.ranges.push((list, low, high));
                p.eat(Punct::Comma);
            }
            if command.ranges.is_empty() {
                return Err(p.unexpected(&["variables with their range, as a (1,5)"]));
            }
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
            use crate::CrosstabsMeasure as M;
            // None is CHISQ; NONE chooses nothing
            const CHOICES: [(&str, &[Option<M>]); 16] = [
                ("CHISQ", &[None]),
                ("PHI", &[Some(M::Phi)]),
                ("CC", &[Some(M::Cc)]),
                ("LAMBDA", &[Some(M::Lambda)]),
                ("BTAU", &[Some(M::Btau)]),
                ("CTAU", &[Some(M::Ctau)]),
                ("GAMMA", &[Some(M::Gamma)]),
                ("CORR", &[Some(M::Corr)]),
                ("KAPPA", &[Some(M::Kappa)]),
                ("RISK", &[Some(M::Risk)]),
                ("UC", &[Some(M::Uc)]),
                ("MCNEMAR", &[Some(M::Mcnemar)]),
                ("D", &[Some(M::D)]),
                ("ETA", &[Some(M::Eta)]),
                ("NONE", &[]),
                // every statistic, CMH with its default odds ratio
                (
                    "ALL",
                    &[
                        None,
                        Some(M::Phi),
                        Some(M::Cc),
                        Some(M::Lambda),
                        Some(M::Btau),
                        Some(M::Ctau),
                        Some(M::Gamma),
                        Some(M::Corr),
                        Some(M::Kappa),
                        Some(M::Risk),
                        Some(M::Uc),
                        Some(M::Mcnemar),
                        Some(M::D),
                        Some(M::Eta),
                        Some(M::Cmh),
                    ],
                ),
            ];
            // without keywords STATISTICS is the chi-square test
            if !p.eat(Punct::Eq) && at_end(p) {
                command.chisq = true;
            } else {
                let unknown = |p: &Parser| {
                    let mut expected = names(&CHOICES);
                    expected.push("CMH");
                    keyword_error(p, "CROSSTABS /STATISTICS", &[], &expected)
                };
                let mut chosen: Vec<Option<M>> = Vec::new();
                let mut any = false;
                while !at_end(p) {
                    any = true;
                    if p.eat_keyword("CMH") {
                        // CMH(value): the common odds ratio of the null hypothesis
                        if p.eat(Punct::LParen) {
                            let start = p.pos;
                            let Some(n) = p.number() else {
                                return Err(p.unexpected(&["a number"]));
                            };
                            if n <= 0.0 {
                                return Err(SyntaxError::new(
                                    p.span_from(start),
                                    format!(
                                        "the common odds ratio of CMH must be above 0, got {n}"
                                    ),
                                ));
                            }
                            p.expect(Punct::RParen)?;
                            command.cmh_odds = n;
                        }
                        chosen.push(Some(M::Cmh));
                    } else {
                        let found = CHOICES.iter().find(|(name, _)| p.at_keyword(name));
                        let Some((_, values)) = found else {
                            return Err(unknown(p));
                        };
                        p.pos += 1;
                        chosen.extend_from_slice(values);
                    }
                    p.eat(Punct::Comma);
                }
                if !any {
                    return Err(unknown(p));
                }
                command.chisq = chosen.contains(&None);
                let mut measures: Vec<M> = chosen.into_iter().flatten().collect();
                measures.sort_unstable();
                measures.dedup();
                command.measures = measures;
            }
        } else if p.eat_keyword("MISSING") {
            p.eat(Punct::Eq);
            const CHOICES: [(&str, &[u8]); 3] =
                [("TABLE", &[0]), ("INCLUDE", &[1]), ("REPORT", &[2])];
            let chosen = keywords(p, &CHOICES, |p| {
                keyword_error(p, "CROSSTABS /MISSING", &[], &names(&CHOICES))
            })?;
            command.include = chosen.last() == Some(&1);
            command.report = chosen.last() == Some(&2);
        } else if p.eat_keyword("COUNT") {
            // a keyword left out is CELL or ROUND, what the dialog writes
            const CHOICES: [(&str, &[u8]); 5] = [
                ("ASIS", &[0]),
                ("CASE", &[1]),
                ("CELL", &[2]),
                ("ROUND", &[3]),
                ("TRUNCATE", &[4]),
            ];
            let chosen = match !p.eat(Punct::Eq) && at_end(p) {
                true => Vec::new(),
                false => keywords(p, &CHOICES, |p| p.unexpected(&names(&CHOICES)))?,
            };
            let last = |of: &[u8]| chosen.iter().rfind(|c| of.contains(*c)).copied();
            command.count = match last(&[0, 1, 2]) {
                Some(0) => WeightCount::Asis,
                Some(1) => WeightCount::Case,
                _ => WeightCount::Cell,
            };
            command.truncate = last(&[3, 4]) == Some(4);
        } else if p.eat_keyword("FORMAT") {
            p.eat(Punct::Eq);
            // the order of the rows, and TABLES (the default) or NOTABLES, the same as CELLS=NONE
            const CHOICES: [(&str, &[u8]); 10] = [
                ("AVALUE", &[0]),
                ("DVALUE", &[1]),
                ("TABLES", &[2]),
                ("NOTABLES", &[3]),
                ("PIVOT", &[4]),
                ("NOPIVOT", &[5]),
                ("BOX", &[6]),
                ("NOBOX", &[7]),
                ("INDEX", &[8]),
                ("NOINDEX", &[9]),
            ];
            let chosen = keywords(p, &CHOICES, |p| p.unexpected(&names(&CHOICES)[..4]))?;
            for &choice in chosen.iter().filter(|&&c| c >= 4) {
                command
                    .layout_words
                    .push(LAYOUT_FORMATS[usize::from(choice) - 4].to_string());
            }
            let last = |pair: [u8; 2]| chosen.iter().rfind(|c| pair.contains(*c)).copied();
            command.descending = last([0, 1]).map_or(command.descending, |c| c == 1);
            no_tables = last([2, 3]).map_or(no_tables, |c| c == 3);
        } else if p.eat_keyword("BARCHART") {
            command.barchart = true;
        } else if p.eat_keyword("WRITE") {
            p.eat(Punct::Eq);
            command.write = if p.eat_keyword("NONE") {
                None
            } else if p.eat_keyword("ALL") {
                Some(CrosstabsWrite::All)
            } else if at_end(p) || p.eat_keyword("CELLS") {
                Some(CrosstabsWrite::Cells)
            } else {
                return Err(p.unexpected(&["NONE", "CELLS", "ALL"]));
            };
        } else if p.eat_keyword("METHOD") {
            p.eat(Punct::Eq);
            command.method = Some(crosstabs_method(p)?);
        } else {
            return Err(p.subcommand_error("CROSSTABS", &CROSSTABS_SUBCOMMANDS));
        }
        more = p.eat(Punct::Slash);
    }
    p.finish(&["'/'"])?;
    if command.tables.is_empty() {
        return Err(p.unexpected(&["TABLES=rows BY columns"]));
    }
    if command.write == Some(CrosstabsWrite::All) && command.ranges.is_empty() {
        let message = "WRITE=ALL works only in integer mode, after VARIABLES with value ranges";
        return Err(SyntaxError::new(p.span_from(0), message));
    }
    if command.report && command.ranges.is_empty() {
        let message =
            "MISSING=REPORT works only in integer mode, after VARIABLES with value ranges";
        return Err(SyntaxError::new(p.span_from(0), message));
    }
    let mut cells = match no_tables {
        true => Vec::new(),
        false => cells.unwrap_or_else(|| vec![CrosstabsCell::Count]),
    };
    cells.sort();
    cells.dedup();
    command.cells = cells;
    Ok(Command::Crosstabs(command))
}
