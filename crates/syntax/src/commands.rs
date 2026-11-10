//! One parser per supported command. Each receives the tokens after the command name.

use crate::ast::{
    Command, Compression, Format, FormatType, Level, MissingValue, Recode, RecodeInput,
    RecodeMapping, RecodeOutput, ValueLabels, VarItem,
};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};

const FILE_SUBCOMMANDS: [&str; 7] = [
    "KEEP",
    "DROP",
    "RENAME",
    "MAP",
    "NAMES",
    "PERMISSIONS",
    "UNSELECTED",
];

const FREQUENCIES_SUBCOMMANDS: [&str; 10] = [
    "FORMAT",
    "MISSING",
    "STATISTICS",
    "PERCENTILES",
    "NTILES",
    "BARCHART",
    "PIECHART",
    "HISTOGRAM",
    "GROUPED",
    "ORDER",
];

/// `[/]KEYWORD[=]'path'`
fn file_path(p: &mut Parser, keyword: &str) -> PResult<String> {
    p.eat(Punct::Slash);
    if !p.eat_keyword(keyword) {
        return Err(p.unexpected(&[keyword]));
    }
    p.eat(Punct::Eq);
    p.string("a quoted file path")
}

pub(crate) fn get(p: &mut Parser) -> PResult<Command> {
    let file = file_path(p, "FILE")?;
    if p.eat(Punct::Slash) {
        return Err(p.subcommand_error("GET", &FILE_SUBCOMMANDS));
    }
    p.finish(&[])?;
    Ok(Command::Get { file })
}

pub(crate) fn save(p: &mut Parser) -> PResult<Command> {
    let outfile = file_path(p, "OUTFILE")?;
    let mut compression = None;
    while p.eat(Punct::Slash) {
        compression = Some(if p.eat_keyword("COMPRESSED") {
            Compression::Compressed
        } else if p.eat_keyword("UNCOMPRESSED") {
            Compression::Uncompressed
        } else if p.eat_keyword("ZCOMPRESSED") {
            Compression::ZCompressed
        } else {
            return Err(p.subcommand_error("SAVE", &FILE_SUBCOMMANDS));
        });
    }
    p.finish(&["'/'"])?;
    Ok(Command::Save {
        outfile,
        compression,
    })
}

pub(crate) fn variable_labels(p: &mut Parser) -> PResult<Command> {
    let mut labels = Vec::new();
    loop {
        let var = p.name("a variable name")?;
        labels.push((var, p.string("a quoted label")?));
        p.eat(Punct::Slash);
        if p.peek().is_none() {
            return Ok(Command::VariableLabels(labels));
        }
    }
}

pub(crate) fn value_labels(p: &mut Parser) -> PResult<Command> {
    let mut groups = Vec::new();
    p.eat(Punct::Slash);
    loop {
        let vars = p.var_list()?;
        let mut labels = Vec::new();
        while let Some(value) = p.value() {
            labels.push((value, p.string("a quoted label")?));
        }
        groups.push(ValueLabels { vars, labels });
        if !p.eat(Punct::Slash) {
            break;
        }
    }
    p.finish(&["a value", "'/'"]).map_err(|e| {
        e.hint("each value needs a quoted label; start another variable group with '/'")
    })?;
    Ok(Command::ValueLabels(groups))
}

/// `varlist (item) [/] varlist (item) ...`
fn groups<T>(
    p: &mut Parser,
    item: impl Fn(&mut Parser) -> PResult<T>,
) -> PResult<Vec<(Vec<VarItem>, T)>> {
    let mut out = Vec::new();
    loop {
        let vars = p.var_list()?;
        p.expect(Punct::LParen)?;
        let value = item(p)?;
        p.expect(Punct::RParen)?;
        out.push((vars, value));
        p.eat(Punct::Slash);
        if p.peek().is_none() {
            return Ok(out);
        }
    }
}

pub(crate) fn missing_values(p: &mut Parser) -> PResult<Command> {
    groups(p, |p| {
        let start = p.pos;
        let mut values = Vec::new();
        while let Some(value) = p.value_or_range()? {
            values.push(value);
            p.eat(Punct::Comma);
        }
        if !p.at(Punct::RParen) {
            return Err(p.unexpected(&["a value", "a range", "')'"]));
        }
        let ranges = values
            .iter()
            .filter(|v| matches!(v, MissingValue::Range(..)))
            .count();
        let limit = if ranges == 0 { 3 } else { 2 };
        if ranges > 1 || values.len() > limit {
            return Err(SyntaxError::new(
                p.span_from(start),
                "too many missing values: at most three values, or one range and one value",
            ));
        }
        Ok(values)
    })
    .map(Command::MissingValues)
}

pub(crate) fn variable_level(p: &mut Parser) -> PResult<Command> {
    groups(p, |p| {
        if p.eat_keyword("SCALE") {
            Ok(Level::Scale)
        } else if p.eat_keyword("ORDINAL") {
            Ok(Level::Ordinal)
        } else if p.eat_keyword("NOMINAL") {
            Ok(Level::Nominal)
        } else {
            Err(p.unexpected(&["SCALE", "ORDINAL", "NOMINAL"]))
        }
    })
    .map(Command::VariableLevel)
}

pub(crate) fn variable_width(p: &mut Parser) -> PResult<Command> {
    groups(p, |p| match p.peek() {
        Some(TokenKind::Number(n)) if n.fract() == 0.0 && *n >= 1.0 && *n <= 1e6 => {
            let n = *n as u32;
            p.pos += 1;
            Ok(n)
        }
        _ => Err(p.unexpected(&["a column width of at least 1"])),
    })
    .map(Command::VariableWidth)
}

/// `F8.2`, `A20`, `DATE11`: type letters, a width and optional decimals.
pub(crate) fn format(text: &str) -> Result<Format, String> {
    let digits = text.find(|c: char| !c.is_ascii_alphabetic());
    let (name, size) = text.split_at(digits.unwrap_or(text.len()));
    let kind = FormatType::from_name(name)
        .ok_or_else(|| format!("unknown format type in {}", text.to_uppercase()))?;
    let (width, decimals) = size.split_once('.').unwrap_or((size, "0"));
    match (width.parse(), decimals.parse()) {
        (Ok(width), Ok(decimals)) => Ok(Format {
            kind,
            width,
            decimals,
        }),
        _ => Err(format!(
            "invalid format {}: write the type, a width and optional decimals",
            text.to_uppercase()
        )),
    }
}

pub(crate) fn formats(p: &mut Parser) -> PResult<Command> {
    groups(p, |p| {
        let example = "a format such as F8.2";
        let Some(TokenKind::Word(word)) = p.peek() else {
            return Err(p.unexpected(&[example]));
        };
        let parsed = format(word).map_err(|message| {
            let mut error = SyntaxError::new(p.span(), message);
            error.expected = vec![example.to_string()];
            error
        })?;
        p.pos += 1;
        Ok(parsed)
    })
    .map(Command::Formats)
}

pub(crate) fn frequencies(p: &mut Parser) -> PResult<Command> {
    p.eat(Punct::Slash);
    // a word is a subcommand only when '=' follows, so the bare form `FREQUENCIES var.` works
    // even for a variable whose name looks like a keyword
    if p.peek_at(1) == Some(&TokenKind::Punct(Punct::Eq)) {
        if !p.at_keyword("VARIABLES") {
            return Err(p.subcommand_error("FREQUENCIES", &FREQUENCIES_SUBCOMMANDS));
        }
        p.pos += 2;
    } else if p.eat_word("VARIABLES") {
        return Err(p.unexpected(&["'='"]));
    }
    let variables = p.var_list()?;
    if p.eat(Punct::Slash) {
        return Err(p.subcommand_error("FREQUENCIES", &FREQUENCIES_SUBCOMMANDS));
    }
    p.finish(&["a variable name", "'/'"])?;
    Ok(Command::Frequencies { variables })
}

pub(crate) fn execute(p: &mut Parser) -> PResult<Command> {
    p.finish(&[])?;
    Ok(Command::Execute)
}

/// `target = expression` or `vector(index) = expression` to the end of the command.
fn assignment(p: &mut Parser) -> PResult<(String, Option<crate::ast::Expr>, crate::ast::Expr)> {
    let target = p.name("a target variable")?;
    let index = if p.eat(Punct::LParen) {
        let index = p.expr()?;
        p.expect(Punct::RParen)?;
        Some(index)
    } else {
        None
    };
    p.expect(Punct::Eq)?;
    let expr = p.expr()?;
    p.finish(&["an operator"])?;
    Ok((target, index, expr))
}

pub(crate) fn compute(p: &mut Parser) -> PResult<Command> {
    Ok(match assignment(p)? {
        (target, None, expr) => Command::Compute { target, expr },
        (vector, Some(index), expr) => Command::VectorAssign {
            condition: None,
            vector,
            index,
            expr,
        },
    })
}

pub(crate) fn if_command(p: &mut Parser) -> PResult<Command> {
    let condition = p.expr()?;
    Ok(match assignment(p)? {
        (target, None, expr) => Command::If {
            condition,
            target,
            expr,
        },
        (vector, Some(index), expr) => Command::VectorAssign {
            condition: Some(condition),
            vector,
            index,
            expr,
        },
    })
}

pub(crate) fn select_if(p: &mut Parser) -> PResult<Command> {
    let condition = p.expr()?;
    p.finish(&["an operator"])?;
    Ok(Command::SelectIf(condition))
}

fn recode_mapping(p: &mut Parser) -> PResult<RecodeMapping> {
    if p.at_keyword("CONVERT") {
        return Err(SyntaxError::new(
            p.span(),
            "CONVERT in RECODE is not supported yet",
        ));
    }
    let mut inputs = Vec::new();
    while !p.at(Punct::Eq) {
        inputs.push(if p.eat_keyword("ELSE") {
            RecodeInput::Else
        } else if p.eat_keyword("MISSING") {
            RecodeInput::Missing
        } else if p.eat_keyword("SYSMIS") {
            RecodeInput::Sysmis
        } else {
            match p.value_or_range()? {
                Some(MissingValue::Value(value)) => RecodeInput::Value(value),
                Some(MissingValue::Range(low, high)) => RecodeInput::Range(low, high),
                None if inputs.is_empty() => {
                    return Err(p.unexpected(&["a value", "a range", "MISSING", "SYSMIS", "ELSE"]));
                }
                None => return Err(p.unexpected(&["a value", "'='"])),
            }
        });
        p.eat(Punct::Comma);
    }
    if inputs.is_empty() {
        return Err(p.unexpected(&["a value", "a range", "MISSING", "SYSMIS", "ELSE"]));
    }
    p.pos += 1;
    let output = if p.eat_keyword("COPY") {
        RecodeOutput::Copy
    } else if p.eat_keyword("SYSMIS") {
        RecodeOutput::Sysmis
    } else if let Some(value) = p.value() {
        RecodeOutput::Value(value)
    } else {
        return Err(p.unexpected(&["a value", "SYSMIS", "COPY"]));
    };
    Ok(RecodeMapping { inputs, output })
}

pub(crate) fn recode(p: &mut Parser) -> PResult<Command> {
    let mut groups = Vec::new();
    loop {
        let vars = p.var_list()?;
        let mut mappings = Vec::new();
        while p.eat(Punct::LParen) {
            mappings.push(recode_mapping(p)?);
            p.expect(Punct::RParen)?;
        }
        if mappings.is_empty() {
            return Err(p.unexpected(&["'('"]));
        }
        let into = if p.eat_keyword("INTO") {
            p.var_list()?
        } else {
            Vec::new()
        };
        groups.push(Recode {
            vars,
            mappings,
            into,
        });
        if !p.eat(Punct::Slash) {
            break;
        }
    }
    p.finish(&["'('", "INTO", "'/'"])?;
    Ok(Command::Recode(groups))
}

/// `(F8.2)`
fn format_in_parens(p: &mut Parser) -> PResult<Format> {
    let example = "a format such as F8.2 or A20";
    p.expect(Punct::LParen)?;
    let Some(TokenKind::Word(word)) = p.peek() else {
        return Err(p.unexpected(&[example]));
    };
    let parsed = format(word).map_err(|message| {
        let mut error = SyntaxError::new(p.span(), message);
        error.expected = vec![example.to_string()];
        error
    })?;
    p.pos += 1;
    p.expect(Punct::RParen)?;
    Ok(parsed)
}

/// `names [(format)] [/ names [(format)] ...]` for the commands that declare new variables.
/// `string` says which kind of format the command takes; only a numeric one may be left out.
fn declarations(p: &mut Parser, string: bool) -> PResult<Vec<(Vec<String>, Option<Format>)>> {
    let mut groups = Vec::new();
    loop {
        let mut names = vec![p.name("a new variable name")?];
        while p.eat(Punct::Comma) || matches!(p.peek(), Some(TokenKind::Word(_))) {
            names.push(p.name("a new variable name")?);
        }
        let start = p.pos;
        let format = if string || p.at(Punct::LParen) {
            Some(format_in_parens(p)?)
        } else {
            None
        };
        let fits = |f: &Format| match string {
            true => f.kind == FormatType::A,
            false => !matches!(f.kind, FormatType::A | FormatType::Ahex),
        };
        if !format.as_ref().is_none_or(fits) {
            let message = if string {
                "STRING needs an A format such as A20"
            } else {
                "NUMERIC needs a numeric format such as F8.2"
            };
            return Err(SyntaxError::new(p.span_from(start), message));
        }
        groups.push((names, format));
        if !p.eat(Punct::Slash) {
            break;
        }
    }
    p.finish(&["'/'"])?;
    Ok(groups)
}

pub(crate) fn string(p: &mut Parser) -> PResult<Command> {
    let groups = declarations(p, true)?;
    let with_format = |(names, format): (_, Option<Format>)| format.map(|f| (names, f));
    Ok(Command::String(
        groups.into_iter().filter_map(with_format).collect(),
    ))
}

pub(crate) fn numeric(p: &mut Parser) -> PResult<Command> {
    declarations(p, false).map(Command::Numeric)
}

pub(crate) fn do_if(p: &mut Parser) -> PResult<Command> {
    let condition = p.expr()?;
    p.finish(&["an operator"])?;
    Ok(Command::DoIf(condition))
}

pub(crate) fn else_if(p: &mut Parser) -> PResult<Command> {
    let condition = p.expr()?;
    p.finish(&["an operator"])?;
    Ok(Command::ElseIf(condition))
}

pub(crate) fn else_command(p: &mut Parser) -> PResult<Command> {
    p.finish(&[])?;
    Ok(Command::Else)
}

pub(crate) fn end_if(p: &mut Parser) -> PResult<Command> {
    p.finish(&[])?;
    Ok(Command::EndIf)
}
