//! The command language: splitting source text into commands and parsing them into an AST.
//!
//! Rules of the language as implemented here (interactive syntax mode):
//!
//! - A command ends with a period that is the last token on a line, at a blank line, or at
//!   the end of input. Only whitespace and `/* */` comments may follow the period, and a
//!   period inside an unclosed `/*` comment does not count.
//! - `* text.` and `COMMENT text.` are comment commands. They run to their terminator, which
//!   is a period as the very last character of a line, or a blank line. They are dropped.
//! - Number tokens are unsigned and `-` is always its own token. Expressions parse it as an
//!   operator; places that take a literal value (value labels, missing values, recode)
//!   accept one `-` directly before a number.
//! - Command names and keywords may be shortened to three or more letters when unambiguous.
//!   Reserved words (AND, OR, NOT, EQ, NE, LT, LE, GT, GE, TO, ALL, BY, WITH) may not.
//! - Names are kept as written; comparing them case-insensitively is the caller's job.

mod ast;
mod commands;
mod ctables;
mod data_commands;
mod data_list;
mod dictionary;
mod encoding;
mod error;
mod examine;
mod expr;
mod files;
mod frequencies;
mod get_data;
mod lexer;
mod loops;
mod macros;
mod mrsets;
mod parser;
mod preprocess;
mod procedures;
mod rank;
mod selection;
mod session_commands;
mod split;
mod stats;
mod table;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_ctables;
#[cfg(test)]
mod tests_data;
#[cfg(test)]
mod tests_data_list;
#[cfg(test)]
mod tests_loops;
#[cfg(test)]
mod tests_preprocess;
#[cfg(test)]
mod tests_procedures;
#[cfg(test)]
mod tests_session;

pub use ast::*;
pub use ctables::{
    Adjust, AxisExpr, AxisVariable, Categories, CategoryItem, CategoryOrder, CompareKind,
    CompareStyle, Comparetest, Ctables, CtablesTable, EmptyCells, Estimate, LabelsMoved,
    PcomputeExpr, PcomputeOp, PercentBase, PercentOf, Pproperties, Sigtest, Statistic, Summary,
    SummaryFunction, VlabelsDisplay,
};
pub use data_list::numbered_names;
pub use encoding::{decode_syntax, encoding_for, legacy_encoding};
pub use error::{Span, SyntaxError};
pub use lexer::{Punct, Token, TokenKind};
pub use loops::{RepeatItem, command_text, substitute};
pub use mrsets::{Mrsets, MrsetsGroup};
pub use preprocess::MacroState;
pub use session_commands::{SETTINGS, SettingEffect};
pub use split::{RawCommand, select_lines, split_commands, split_commands_in, split_commands_with};
pub use table::{
    CommandSpec, command_name, command_names, command_specs, help, unsupported_commands,
};

use parser::Parser;
use table::Resolution;

/// A parsed command with the lines it occupies (1-based, inclusive).
#[derive(Debug, Clone, PartialEq)]
pub struct Parsed {
    pub command: Command,
    pub first_line: u32,
    pub last_line: u32,
}

/// Parses every command of the source. A bad command adds one error and parsing goes on with
/// the next command, so the result holds all good commands and one error per bad one.
pub fn parse(source: &str) -> (Vec<Parsed>, Vec<SyntaxError>) {
    let mut parsed = Vec::new();
    let mut errors = Vec::new();
    let mut raws = Vec::new();
    for raw in split::split(source, None) {
        match raw {
            Ok(raw) => raws.push(raw),
            Err(error) => errors.push(error),
        }
    }
    for (range, result) in parse_raws(&raws) {
        match result {
            Ok(command) => parsed.push(Parsed {
                command,
                first_line: raws[range.start].first_line,
                last_line: raws[range.end - 1].last_line,
            }),
            Err(error) => errors.push(error),
        }
    }
    (parsed, errors)
}

/// Parses commands in order, each with the range of `raws` it takes. DO REPEAT takes the
/// commands up to its END REPEAT as its body, which is checked here with the first item of
/// every replacement list.
pub fn parse_raws(
    raws: &[RawCommand],
) -> Vec<(std::ops::Range<usize>, Result<Command, SyntaxError>)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < raws.len() {
        if raws[i].name != "DO REPEAT" {
            out.push((i..i + 1, parse_command(&raws[i])));
            i += 1;
            continue;
        }
        let mut depth = 0;
        let end = (i + 1..raws.len()).find(|&j| match raws[j].name.as_str() {
            "DO REPEAT" => {
                depth += 1;
                false
            }
            "END REPEAT" if depth == 0 => true,
            "END REPEAT" => {
                depth -= 1;
                false
            }
            _ => false,
        });
        let Some(end) = end else {
            let start = raws[i].tokens.first().map_or(raws[i].end, |t| t.span);
            let error = SyntaxError::new(start, "DO REPEAT without END REPEAT")
                .hint("end the commands to repeat with END REPEAT");
            out.push((i..i + 1, Err(error)));
            i += 1;
            continue;
        };
        let mut repeat = match parse_command(&raws[i]) {
            Ok(Command::DoRepeat(repeat)) => repeat,
            other => {
                out.push((i..end + 1, other));
                i = end + 1;
                continue;
            }
        };
        repeat.print =
            raws[end].tokens.iter().skip(2).any(
                |t| matches!(&t.kind, TokenKind::Word(w) if table::matches_keyword(w, "PRINT")),
            );
        repeat.body = raws[i + 1..end].to_vec();
        let items: Vec<(String, RepeatItem)> = repeat
            .lists
            .iter()
            .map(|(name, list)| (name.clone(), loops::sample_item(list)))
            .collect();
        let items: Vec<(&str, &RepeatItem)> =
            items.iter().map(|(n, item)| (n.as_str(), item)).collect();
        let body: Vec<RawCommand> = repeat.body.iter().map(|r| substitute(r, &items)).collect();
        let errors: Vec<_> = parse_raws(&body)
            .into_iter()
            .filter_map(|(_, result)| result.err())
            .collect();
        if errors.is_empty() {
            out.push((i..end + 1, Ok(Command::DoRepeat(repeat))));
        } else {
            out.extend(errors.into_iter().map(|e| (i..end + 1, Err(e))));
        }
        i = end + 1;
    }
    out
}

fn or_list(names: &[&str]) -> String {
    match names {
        [rest @ .., last] if !rest.is_empty() => format!("{} or {last}", rest.join(", ")),
        _ => names.concat(),
    }
}

/// Parses the body of one command from `split_commands`.
pub fn parse_command(raw: &RawCommand) -> Result<Command, SyntaxError> {
    let error = |error| split::multiline_hint(&raw.tokens, error);
    parse_raw(raw)
        .map_err(error)
        .map_err(|error| match &raw.origin {
            Some(origin) if error.hint.is_none() => error.hint(format!("in {origin}")),
            _ => error,
        })
}

fn parse_raw(raw: &RawCommand) -> Result<Command, SyntaxError> {
    let Some(first) = raw.tokens.first() else {
        return Err(SyntaxError::new(raw.end, "empty command"));
    };
    let name_span = |used: usize| first.span.to(raw.tokens[used - 1].span);
    match table::resolve_tokens(&raw.tokens) {
        Resolution::Supported(spec, used) => {
            (spec.parse)(&mut Parser::new(&raw.tokens, used, raw.end))
        }
        Resolution::Unsupported("END REPEAT", used) => Err(SyntaxError::new(
            name_span(used),
            "END REPEAT without DO REPEAT",
        )
        .hint("the commands to repeat start with DO REPEAT")),
        Resolution::Unsupported("END DATA", used) => Err(SyntaxError::new(
            name_span(used),
            "END DATA without BEGIN DATA",
        )
        .hint("inline data starts with a line that holds only BEGIN DATA")),
        Resolution::Unsupported(name, used) => Err(SyntaxError::new(
            name_span(used),
            format!("command {name} is not supported yet"),
        )
        .hint(format!(
            "supported commands: {}",
            command_names().join(", ")
        ))),
        Resolution::Ambiguous(names) => {
            let mut error = SyntaxError::new(
                first.span,
                format!("ambiguous command abbreviation {}", raw.name),
            );
            error.expected = names.iter().map(|name| name.to_string()).collect();
            Err(error)
        }
        Resolution::Unknown => {
            let words = table::name_words(&raw.tokens);
            let words: Vec<&str> = words.iter().map(|(word, _)| word.as_str()).collect();
            if words.is_empty() {
                let mut error = SyntaxError::new(first.span, "a command must start with its name");
                error.expected = vec!["a command name".to_string()];
                return Err(error);
            }
            let error = SyntaxError::new(
                first.span,
                format!("unknown command {}", words[0].to_uppercase()),
            );
            let suggestions = table::suggest(&words);
            Err(if suggestions.is_empty() {
                error
            } else {
                error.hint(format!("did you mean {}?", or_list(&suggestions)))
            })
        }
    }
}
