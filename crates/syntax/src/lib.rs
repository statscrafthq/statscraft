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
mod error;
mod expr;
mod files;
mod lexer;
mod mrsets;
mod parser;
mod procedures;
mod split;
mod table;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_ctables;
#[cfg(test)]
mod tests_data;
#[cfg(test)]
mod tests_procedures;

pub use ast::*;
pub use ctables::{
    Adjust, AxisExpr, AxisVariable, Categories, CategoryItem, CompareKind, CompareStyle,
    Comparetest, Ctables, CtablesTable, EmptyCells, LabelsMoved, PcomputeExpr, PcomputeOp,
    PercentBase, PercentOf, Pproperties, Sigtest, Statistic, Summary, SummaryFunction,
    VlabelsDisplay,
};
pub use error::{Span, SyntaxError};
pub use lexer::{Punct, Token, TokenKind};
pub use mrsets::{Mrsets, MrsetsGroup};
pub use split::{RawCommand, select_lines, split_commands};
pub use table::{command_names, help};

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
    for raw in split::split(source) {
        match raw.and_then(|raw| Ok((parse_command(&raw)?, raw))) {
            Ok((command, raw)) => parsed.push(Parsed {
                command,
                first_line: raw.first_line,
                last_line: raw.last_line,
            }),
            Err(error) => errors.push(error),
        }
    }
    (parsed, errors)
}

fn or_list(names: &[&str]) -> String {
    match names {
        [rest @ .., last] if !rest.is_empty() => format!("{} or {last}", rest.join(", ")),
        _ => names.concat(),
    }
}

/// Parses the body of one command from `split_commands`.
pub fn parse_command(raw: &RawCommand) -> Result<Command, SyntaxError> {
    let Some(first) = raw.tokens.first() else {
        return Err(SyntaxError::new(raw.end, "empty command"));
    };
    let name_span = |used: usize| first.span.to(raw.tokens[used - 1].span);
    match table::resolve_tokens(&raw.tokens) {
        Resolution::Supported(spec, used) => {
            (spec.parse)(&mut Parser::new(&raw.tokens, used, raw.end))
        }
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
