//! Parsers of the data management commands: COUNT, SORT CASES, RENAME VARIABLES, DELETE
//! VARIABLES, WEIGHT and AGGREGATE.

use crate::ast::{
    Aggregate, AggregateFunction, Aggregation, Autorecode, Command, Count, MissingValue,
    RecodeInput, SortKey, VarItem,
};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::table::matches_keyword;

const AGGREGATE_SUBCOMMANDS: [&str; 3] = ["PRESORTED", "DOCUMENT", "MISSING"];

const AGGREGATE_FUNCTIONS: [(&str, AggregateFunction); 12] = [
    ("SUM", AggregateFunction::Sum),
    ("MEAN", AggregateFunction::Mean),
    ("SD", AggregateFunction::Sd),
    ("MEDIAN", AggregateFunction::Median),
    ("MIN", AggregateFunction::Min),
    ("MAX", AggregateFunction::Max),
    ("FIRST", AggregateFunction::First),
    ("LAST", AggregateFunction::Last),
    ("N", AggregateFunction::N),
    ("NU", AggregateFunction::Nu),
    ("NMISS", AggregateFunction::Nmiss),
    ("NUMISS", AggregateFunction::Numiss),
];

fn at_word(p: &Parser) -> bool {
    matches!(p.peek(), Some(TokenKind::Word(_)))
}

/// `(values)` of COUNT: values, ranges, MISSING and SYSMIS, with optional commas.
fn count_values(p: &mut Parser) -> PResult<Vec<RecodeInput>> {
    const EXPECTED: [&str; 4] = ["a value", "a range", "MISSING", "SYSMIS"];
    p.expect(Punct::LParen)?;
    let mut values = Vec::new();
    while !p.at(Punct::RParen) {
        values.push(if p.eat_keyword("MISSING") {
            RecodeInput::Missing
        } else if p.eat_keyword("SYSMIS") {
            RecodeInput::Sysmis
        } else {
            match p.value_or_range()? {
                Some(MissingValue::Value(value)) => RecodeInput::Value(value),
                Some(MissingValue::Range(low, high)) => RecodeInput::Range(low, high),
                None if values.is_empty() => return Err(p.unexpected(&EXPECTED)),
                None => return Err(p.unexpected(&["a value", "')'"])),
            }
        });
        p.eat(Punct::Comma);
    }
    if values.is_empty() {
        return Err(p.unexpected(&EXPECTED));
    }
    p.pos += 1;
    Ok(values)
}

pub(crate) fn count(p: &mut Parser) -> PResult<Command> {
    let mut counts = Vec::new();
    loop {
        let target = p.name("a target variable")?;
        p.expect(Punct::Eq)?;
        let mut criteria = Vec::new();
        loop {
            let vars = p.var_list()?;
            criteria.push((vars, count_values(p)?));
            if !at_word(p) {
                break;
            }
        }
        counts.push(Count { target, criteria });
        if !p.eat(Punct::Slash) {
            break;
        }
    }
    p.finish(&["a variable name", "'/'"])?;
    Ok(Command::Count(counts))
}

/// `varlist [(A|D)] [varlist [(A|D)] ...]`. A direction covers the variables listed before
/// it, back to the previous direction.
fn sort_keys(p: &mut Parser) -> PResult<Vec<SortKey>> {
    let mut keys = Vec::new();
    loop {
        let vars = p.var_list()?;
        let mut descending = false;
        if p.eat(Punct::LParen) {
            if p.eat_word("D") || p.eat_keyword("DOWN") {
                descending = true;
            } else if !(p.eat_word("A") || p.eat_keyword("UP")) {
                return Err(p.unexpected(&["A", "D"]));
            }
            p.expect(Punct::RParen)?;
        }
        keys.push(SortKey { vars, descending });
        if !at_word(p) {
            return Ok(keys);
        }
    }
}

pub(crate) fn sort_cases(p: &mut Parser) -> PResult<Command> {
    p.eat_word("BY");
    let keys = sort_keys(p)?;
    p.finish(&["a variable name", "'('"])?;
    Ok(Command::SortCases(keys))
}

pub(crate) fn rename_variables(p: &mut Parser) -> PResult<Command> {
    let mut groups = Vec::new();
    loop {
        if p.eat(Punct::LParen) {
            let old = p.var_list()?;
            p.expect(Punct::Eq)?;
            let new = p.var_list()?;
            p.expect(Punct::RParen)?;
            groups.push((old, new));
        } else {
            let old = p.name("a variable name")?;
            p.expect(Punct::Eq).map_err(|e| {
                e.hint("to rename several variables at once write (old1 old2 = new1 new2)")
            })?;
            let new = p.name("a new variable name")?;
            groups.push((vec![VarItem::Name(old)], vec![VarItem::Name(new)]));
        }
        p.eat(Punct::Slash);
        if p.peek().is_none() {
            return Ok(Command::RenameVariables(groups));
        }
    }
}

pub(crate) fn delete_variables(p: &mut Parser) -> PResult<Command> {
    let vars = p.var_list()?;
    p.finish(&["a variable name"])?;
    Ok(Command::DeleteVariables(vars))
}

pub(crate) fn weight(p: &mut Parser) -> PResult<Command> {
    // after BY the word OFF is a variable name
    let variable = if !p.eat_word("BY") && p.eat_word("OFF") {
        None
    } else {
        Some(p.name("a variable name")?)
    };
    p.finish(&[])?;
    Ok(Command::Weight(variable))
}

/// `target ['label'] [target ['label'] ...] = FUNCTION[(varlist)]`
fn aggregation(p: &mut Parser) -> PResult<Aggregation> {
    let mut targets = Vec::new();
    while !p.at(Punct::Eq) {
        let first = p.name("a target variable")?;
        let item = if p.eat_word("TO") {
            VarItem::Range(first, p.name("a target variable")?)
        } else {
            VarItem::Name(first)
        };
        let label = match p.peek() {
            Some(TokenKind::Str(_)) => Some(p.string("a quoted label")?),
            _ => None,
        };
        targets.push((item, label));
        p.eat(Punct::Comma);
    }
    if targets.is_empty() {
        return Err(p.unexpected(&["a target variable"]));
    }
    p.pos += 1;
    let start = p.pos;
    let name = p.name("an aggregate function")?;
    let known = AGGREGATE_FUNCTIONS
        .iter()
        .find(|(known, _)| known.eq_ignore_ascii_case(&name));
    let Some((_, function)) = known else {
        let message = format!(
            "aggregate function {} is not supported",
            name.to_uppercase()
        );
        let mut error = SyntaxError::new(p.span_from(start), message);
        error.expected = AGGREGATE_FUNCTIONS.map(|(name, _)| name.to_string()).into();
        return Err(error);
    };
    let sources = if p.eat(Punct::LParen) {
        let sources = p.var_list()?;
        p.expect(Punct::RParen)?;
        sources
    } else if matches!(function, AggregateFunction::N | AggregateFunction::Nu) {
        Vec::new()
    } else {
        return Err(p.unexpected(&["'('"]));
    };
    Ok(Aggregation {
        targets,
        function: *function,
        sources,
    })
}

pub(crate) fn aggregate(p: &mut Parser) -> PResult<Command> {
    let start = p.pos;
    p.eat(Punct::Slash);
    if !p.eat_keyword("OUTFILE") {
        // without OUTFILE the results are added to the active dataset (manual, basic specification)
        p.pos = start;
        return aggregate_body(p, None, true);
    }
    p.eat(Punct::Eq);
    let outfile = if p.eat(Punct::Star) {
        None
    } else {
        Some(crate::files::dataset_or_file(p)?)
    };
    let mut add_variables = false;
    if p.eat_keyword("MODE") {
        p.eat(Punct::Eq);
        if p.eat_keyword("ADDVARIABLES") {
            add_variables = true;
        } else if !p.eat_keyword("REPLACE") {
            return Err(p.unexpected(&["ADDVARIABLES", "REPLACE"]));
        }
    }
    aggregate_body(p, outfile, add_variables)
}

fn aggregate_body(
    p: &mut Parser,
    outfile: Option<String>,
    add_variables: bool,
) -> PResult<Command> {
    let mut breaks = Vec::new();
    let mut aggregations = Vec::new();
    // the first subcommand may come without its slash, as in `AGGREGATE BREAK=g /m = MEAN(x)`
    let mut first = true;
    while p.eat(Punct::Slash) || (std::mem::take(&mut first) && p.peek().is_some()) {
        first = false;
        let assigns = p.peek_at(1) == Some(&TokenKind::Punct(Punct::Eq));
        // these words are subcommands only where they cannot start a target list
        let bare = matches!(p.peek_at(1), None | Some(TokenKind::Punct(Punct::Slash)));
        let columnwise =
            matches!(p.peek_at(2), Some(TokenKind::Word(w)) if matches_keyword(w, "COLUMNWISE"));
        // PRESORTED only promises sorted cases: groups by the break values come out the same
        if bare && p.at_keyword("PRESORTED") {
            p.pos += 1;
            continue;
        }
        if (bare && p.at_keyword("DOCUMENT")) || (assigns && columnwise && p.at_keyword("MISSING"))
        {
            return Err(p.subcommand_error("AGGREGATE", &AGGREGATE_SUBCOMMANDS));
        }
        if assigns && p.at_keyword("BREAK") {
            p.pos += 2;
            breaks.extend(sort_keys(p)?);
        } else {
            aggregations.push(aggregation(p)?);
        }
    }
    p.finish(&["'/'"])?;
    if aggregations.is_empty() {
        return Err(p.unexpected(&["/target = FUNCTION(variable)"]));
    }
    Ok(Command::Aggregate(Aggregate {
        outfile,
        add_variables,
        breaks,
        aggregations,
    }))
}

pub(crate) fn autorecode(p: &mut Parser) -> PResult<Command> {
    p.eat(Punct::Slash);
    if p.eat_keyword("VARIABLES") {
        p.eat(Punct::Eq);
    }
    let variables = p.var_list()?;
    p.expect(Punct::Slash)?;
    if !p.eat_keyword("INTO") {
        return Err(p.unexpected(&["INTO"]));
    }
    p.eat(Punct::Eq);
    let mut command = Autorecode {
        variables,
        into: p.var_list()?,
        blank_missing: false,
        group: false,
        descending: false,
        print: false,
    };
    while p.eat(Punct::Slash) {
        if p.eat_keyword("BLANK") {
            p.eat(Punct::Eq);
            if p.eat_keyword("MISSING") {
                command.blank_missing = true;
            } else if !p.eat_keyword("VALID") {
                return Err(p.unexpected(&["VALID", "MISSING"]));
            }
        } else if p.eat_keyword("GROUP") {
            command.group = true;
        } else if p.eat_keyword("DESCENDING") {
            command.descending = true;
        } else if p.eat_keyword("PRINT") {
            command.print = true;
        } else {
            return Err(p.subcommand_error("AUTORECODE", &["APPLY", "SAVE"]));
        }
    }
    p.finish(&["'/'"])?;
    Ok(Command::Autorecode(command))
}
