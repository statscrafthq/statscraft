//! Splitting source text into commands and selecting commands by line range.

use crate::error::{Span, SyntaxError};
use crate::lexer::{Lexer, Token, TokenKind};
use crate::table::{Resolution, matches_keyword, resolve_tokens};

/// One command as found in the source, before its body is parsed.
#[derive(Debug, Clone, PartialEq)]
pub struct RawCommand {
    /// The canonical name, for example "VARIABLE LABELS". For an unknown command: its first
    /// word in upper case, or empty when the command does not start with a word.
    pub name: String,
    /// The line of the first token. Comment-only lines above the command do not count.
    pub first_line: u32,
    /// The line that ends the command: the line with the terminating period, or the last
    /// line before a blank line or the end of input.
    pub last_line: u32,
    /// All tokens, name words included. The body of a command that is not supported is not
    /// tokenised, because commands such as TITLE hold free text.
    pub tokens: Vec<Token>,
    /// Where the command ends: the terminating period, or just after the last line.
    pub end: Span,
}

/// Enough tokens to hold the longest command name.
const NAME_TOKENS: usize = 5;

/// The position of the period that terminates a command on this line, as (byte index,
/// 1-based column). In a comment command only a period that is the very last character counts;
/// elsewhere the period may be followed by `/* */` comments and can hide inside one.
fn terminator(line: &str, in_comment_command: bool) -> Option<(usize, u32)> {
    if in_comment_command {
        let code = line.trim_end();
        return code
            .ends_with('.')
            .then(|| (code.len() - 1, code.chars().count() as u32));
    }
    let chars: Vec<(usize, char)> = line.char_indices().collect();
    let mut last = None;
    let mut quote = None;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i].1;
        if quote == Some(c) {
            quote = None;
        } else if quote.is_none() {
            if c == '/' && chars.get(i + 1).is_some_and(|next| next.1 == '*') {
                let close = (i + 2..chars.len().saturating_sub(1))
                    .find(|&k| chars[k].1 == '*' && chars[k + 1].1 == '/');
                i = close.map_or(chars.len(), |k| k + 2);
                continue;
            }
            if c == '\'' || c == '"' {
                quote = Some(c);
            }
        }
        if !c.is_whitespace() {
            last = Some(i);
        }
        i += 1;
    }
    last.filter(|&i| chars[i].1 == '.')
        .map(|i| (chars[i].0, i as u32 + 1))
}

/// Whether a command whose first code is on this line is a comment command (`*` or COMMENT).
/// None when the line holds no code at all.
fn starts_comment(line: &str) -> Option<bool> {
    let lines = [(0, line)];
    let mut lexer = Lexer::new(&lines);
    if lexer.peek_char()? == '*' {
        return Some(true);
    }
    // COMMENT needs four letters, so that COM stays an abbreviation of COMPUTE
    Some(matches!(
        lexer.next_token(),
        Ok(Some(Token { kind: TokenKind::Word(w), .. })) if w.len() >= 4 && matches_keyword(&w, "COMMENT")
    ))
}

fn build(lines: &[(u32, &str)], end: Span) -> Option<Result<RawCommand, SyntaxError>> {
    let mut lexer = Lexer::new(lines);
    let mut tokens = Vec::new();
    let mut lex_error = None;
    while tokens.len() < NAME_TOKENS && lex_error.is_none() {
        match lexer.next_token() {
            Ok(Some(token)) => tokens.push(token),
            Ok(None) => break,
            Err(error) => lex_error = Some(error),
        }
    }
    let name = match resolve_tokens(&tokens) {
        Resolution::Supported(spec, _) => {
            lexer.glued_words = spec.name == "DATA LIST";
            // only a command with a parser has a body worth tokenising
            while lex_error.is_none() {
                match lexer.next_token() {
                    Ok(Some(token)) => tokens.push(token),
                    Ok(None) => break,
                    Err(error) => lex_error = Some(error),
                }
            }
            if let Some(error) = lex_error {
                return Some(Err(error));
            }
            spec.name.to_string()
        }
        Resolution::Unsupported(name, used) => {
            tokens.truncate(used);
            name.to_string()
        }
        _ => match tokens.first().map(|t| &t.kind) {
            Some(TokenKind::Word(w)) => w.to_uppercase(),
            Some(_) => String::new(),
            None => return lex_error.map(Err),
        },
    };
    Some(Ok(RawCommand {
        name,
        first_line: tokens[0].span.line,
        last_line: lines[lines.len() - 1].0,
        tokens,
        end,
    }))
}

/// True for a line that holds only the words of `name` (two words separated by blanks),
/// optionally followed by a period. Inline data starts and ends with such lines.
fn is_line_command(line: &str, name: [&str; 2]) -> bool {
    let line = line.trim();
    let line = line.strip_suffix('.').unwrap_or(line);
    let mut words = line.split_whitespace();
    words
        .next()
        .is_some_and(|w| w.eq_ignore_ascii_case(name[0]))
        && words
            .next()
            .is_some_and(|w| w.eq_ignore_ascii_case(name[1]))
        && words.next().is_none()
}

/// The BEGIN DATA command on line index `first` with its data lines, up to the END DATA line,
/// and the index of the line after END DATA. Each data line becomes a string token on its own
/// line, so the lines keep their numbers.
fn inline_data(lines: &[&str], first: usize) -> (Result<RawCommand, SyntaxError>, usize) {
    let line_span = |index: usize, text: &str| Span {
        line: index as u32 + 1,
        column: 1,
        end_line: index as u32 + 1,
        end_column: text.chars().count() as u32 + 1,
    };
    let Some(end) = (first + 1..lines.len()).find(|&i| is_line_command(lines[i], ["END", "DATA"]))
    else {
        let error = SyntaxError::new(
            line_span(first, lines[first]),
            "BEGIN DATA without END DATA",
        )
        .hint("end the data with a line that holds only END DATA");
        return (Err(error), lines.len());
    };
    let begin = lines[first];
    let word = |word: &str| {
        let at = begin.to_uppercase().find(word).unwrap_or(0);
        let column = begin[..at].chars().count() as u32 + 1;
        Token {
            kind: TokenKind::Word(word.to_string()),
            span: Span {
                line: first as u32 + 1,
                column,
                end_line: first as u32 + 1,
                end_column: column + word.len() as u32,
            },
        }
    };
    let mut tokens = vec![word("BEGIN"), word("DATA")];
    for (index, text) in lines.iter().enumerate().take(end).skip(first + 1) {
        tokens.push(Token {
            kind: TokenKind::Str(text.to_string()),
            span: line_span(index, text),
        });
    }
    let command = RawCommand {
        name: "BEGIN DATA".to_string(),
        first_line: first as u32 + 1,
        last_line: end as u32 + 1,
        tokens,
        end: line_span(end, lines[end]),
    };
    (Ok(command), end + 1)
}

/// Every command of the source, or the error that prevented tokenising it.
pub(crate) fn split(source: &str) -> Vec<Result<RawCommand, SyntaxError>> {
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    // the extra blank line ends a command that runs to the end of input
    let lines: Vec<&str> = source.lines().chain([""]).collect();
    let mut out = Vec::new();
    let mut current: Vec<(u32, &str)> = Vec::new();
    let mut comment = None;
    let mut index = 0;
    while index < lines.len() {
        let (line, number) = (lines[index], index as u32 + 1);
        index += 1;
        // inline data: the lines up to END DATA are data, whatever they look like
        if comment != Some(true) && is_line_command(line, ["BEGIN", "DATA"]) {
            if let Some(&(last, text)) = current.last() {
                let column = text.trim_end().chars().count() as u32 + 1;
                let end = Span {
                    line: last,
                    column,
                    end_line: last,
                    end_column: column,
                };
                out.extend(build(&current, end));
            }
            current.clear();
            comment = None;
            let (command, next) = inline_data(&lines, index - 1);
            out.push(command);
            index = next;
            continue;
        }
        let end = if line.trim().is_empty() {
            let Some(&(last, text)) = current.last() else {
                continue;
            };
            let column = text.trim_end().chars().count() as u32 + 1;
            Span {
                line: last,
                column,
                end_line: last,
                end_column: column,
            }
        } else {
            comment = comment.or_else(|| starts_comment(line));
            let Some((byte, column)) = terminator(line, comment == Some(true)) else {
                current.push((number, line));
                continue;
            };
            current.push((number, &line[..byte]));
            Span {
                line: number,
                column,
                end_line: number,
                end_column: column + 1,
            }
        };
        if comment != Some(true)
            && let Some(command) = build(&current, end)
        {
            out.push(command);
        }
        current.clear();
        comment = None;
    }
    out
}

/// Splits the source into commands with their exact line ranges. Comment commands and empty
/// commands are left out. Fails on the first command that cannot be tokenised (for example an
/// unterminated string); an unknown command name is not an error here but in `parse_command`.
pub fn split_commands(source: &str) -> Result<Vec<RawCommand>, SyntaxError> {
    split(source).into_iter().collect()
}

fn lines(first: u32, last: u32) -> String {
    if first == last {
        format!("line {first}")
    } else {
        format!("lines {first}-{last}")
    }
}

/// The commands that lie fully inside the 1-based inclusive line range, which may be none.
/// Fails when the range cuts through a command, naming that command and its real line range.
/// `commands` must be in source order, as `split_commands` returns them.
pub fn select_lines(
    commands: &[RawCommand],
    first: u32,
    last: u32,
) -> Result<&[RawCommand], SyntaxError> {
    if first == 0 || last < first {
        let span = Span {
            line: first,
            column: 1,
            end_line: last,
            end_column: 1,
        };
        return Err(SyntaxError::new(
            span,
            format!(
                "invalid line range {first}-{last}: lines start at 1 and the range must not be reversed"
            ),
        ));
    }
    let inside = |c: &&RawCommand| first <= c.first_line && c.last_line <= last;
    let touches = |c: &&RawCommand| c.first_line <= last && c.last_line >= first;
    if let Some(cut) = commands.iter().find(|c| touches(c) && !inside(c)) {
        let start = cut.tokens.first().map_or(cut.end, |t| t.span);
        let message = format!(
            "line range {first}-{last} cuts through command {}, which occupies {}",
            cut.name,
            lines(cut.first_line, cut.last_line)
        );
        return Err(SyntaxError::new(start.to(cut.end), message).hint(format!(
            "use {} to include the whole command",
            lines(first.min(cut.first_line), last.max(cut.last_line))
        )));
    }
    let start = commands.iter().position(|c| inside(&c)).unwrap_or(0);
    let count = commands[start..].iter().take_while(inside).count();
    Ok(&commands[start..start + count])
}
