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
/// elsewhere the period may be followed by `/* */` comments and can hide inside one, or inside
/// a string. `quote` is the string left open by the lines before; on return it holds the string
/// this line leaves open; the caller decides whether a period inside it counts.
fn terminator(
    line: &str,
    in_comment_command: bool,
    quote: &mut Option<char>,
) -> Option<(usize, u32)> {
    if in_comment_command {
        let code = line.trim_end();
        return code
            .ends_with('.')
            .then(|| (code.len() - 1, code.chars().count() as u32));
    }
    let chars: Vec<(usize, char)> = line.char_indices().collect();
    let mut last = None;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i].1;
        if *quote == Some(c) {
            *quote = None;
        } else if quote.is_none() {
            if c == '/' && chars.get(i + 1).is_some_and(|next| next.1 == '*') {
                let close = (i + 2..chars.len().saturating_sub(1))
                    .find(|&k| chars[k].1 == '*' && chars[k + 1].1 == '/');
                i = close.map_or(chars.len(), |k| k + 2);
                continue;
            }
            if c == '\'' || c == '"' {
                *quote = Some(c);
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

/// Commands whose body is free text. It is kept as written, as one string token, and a quote
/// in it does not open a string that runs on to the next line.
const FREE_TEXT: [&str; 4] = ["DOCUMENT", "FILE LABEL", "SUBTITLE", "TITLE"];

/// How a command whose first code is on a line is read.
#[derive(Clone, Copy, PartialEq)]
enum Start {
    /// `*` or COMMENT: runs to a period that is the last character of a line.
    Comment,
    /// One of `FREE_TEXT`, or a command that is no command of the language: a quote does not
    /// open a string beyond its line. Only DOCUMENT keeps blank lines, as paragraph breaks.
    PerLine { document: bool },
    /// Every other command: a string may run on over line breaks.
    Code,
}

/// How the command whose first code is on this line is read. None when the line holds no
/// code at all.
fn start(line: &str) -> Option<Start> {
    let lines = [(0, line)];
    let mut lexer = Lexer::new(&lines);
    if lexer.peek_char()? == '*' {
        return Some(Start::Comment);
    }
    let mut tokens = Vec::new();
    while let (true, Ok(Some(token))) = (tokens.len() < NAME_TOKENS, lexer.next_token()) {
        tokens.push(token);
    }
    // COMMENT needs four letters, so that COM stays an abbreviation of COMPUTE
    if let Some(Token {
        kind: TokenKind::Word(w),
        ..
    }) = tokens.first()
        && w.len() >= 4
        && matches_keyword(w, "COMMENT")
    {
        return Some(Start::Comment);
    }
    let name = match resolve_tokens(&tokens) {
        Resolution::Supported(spec, _) => spec.name,
        Resolution::Unsupported(name, _) => name,
        _ => return Some(Start::PerLine { document: false }),
    };
    Some(match FREE_TEXT.contains(&name) {
        true => Start::PerLine {
            document: name == "DOCUMENT",
        },
        false => Start::Code,
    })
}

/// The text of a free-text command after its name, lines joined with line breaks, without the
/// blank space at either end.
fn free_text(lines: &[(u32, &str)], name_end: Span) -> String {
    let mut text = String::new();
    let mut started = false;
    for &(number, line) in lines {
        if number < name_end.end_line {
            continue;
        }
        if !started {
            text.extend(line.chars().skip(name_end.end_column as usize - 1));
            started = true;
        } else {
            text.push('\n');
            text.push_str(line);
        }
    }
    text.trim().to_string()
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
        // a quoted FILE LABEL follows the rules for strings
        Resolution::Supported(spec, used)
            if FREE_TEXT.contains(&spec.name)
                && !(spec.name == "FILE LABEL"
                    && matches!(tokens.get(used).map(|t| &t.kind), Some(TokenKind::Str(_)))) =>
        {
            tokens.truncate(used);
            let name_end = tokens[used - 1].span;
            tokens.push(Token {
                kind: TokenKind::Str(free_text(lines, name_end)),
                span: Span {
                    line: name_end.end_line,
                    column: name_end.end_column,
                    ..end
                },
            });
            spec.name.to_string()
        }
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
            // END REPEAT keeps its PRINT for the DO REPEAT that it ends
            if name != "END REPEAT" {
                tokens.truncate(used);
            }
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
pub(crate) fn is_line_command(line: &str, name: [&str; 2]) -> bool {
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
fn inline_data(lines: &[(u32, &str)], first: usize) -> (Result<RawCommand, SyntaxError>, usize) {
    let line_span = |(number, text): (u32, &str)| Span {
        line: number,
        column: 1,
        end_line: number,
        end_column: text.chars().count() as u32 + 1,
    };
    let Some(end) =
        (first + 1..lines.len()).find(|&i| is_line_command(lines[i].1, ["END", "DATA"]))
    else {
        let error = SyntaxError::new(line_span(lines[first]), "BEGIN DATA without END DATA")
            .hint("end the data with a line that holds only END DATA");
        return (Err(error), lines.len());
    };
    let (number, begin) = lines[first];
    let word = |word: &str| {
        let at = begin.to_uppercase().find(word).unwrap_or(0);
        let column = begin[..at].chars().count() as u32 + 1;
        Token {
            kind: TokenKind::Word(word.to_string()),
            span: Span {
                line: number,
                column,
                end_line: number,
                end_column: column + word.len() as u32,
            },
        }
    };
    let mut tokens = vec![word("BEGIN"), word("DATA")];
    for &(line, text) in &lines[first + 1..end] {
        tokens.push(Token {
            kind: TokenKind::Str(text.to_string()),
            span: line_span((line, text)),
        });
    }
    let command = RawCommand {
        name: "BEGIN DATA".to_string(),
        first_line: number,
        last_line: lines[end].0,
        tokens,
        end: line_span(lines[end]),
    };
    (Ok(command), end + 1)
}

/// The end of the command whose first line is `from` (not blank): the index after its last
/// line, the terminator on that line when a period ended the command, and how it is read.
/// A blank line ends a command, except inside a string or in DOCUMENT.
pub(crate) fn command_end(
    lines: &[(u32, &str)],
    from: usize,
) -> (usize, Option<(usize, u32)>, Start) {
    let kind = start(lines[from].1).unwrap_or(Start::Code);
    // a string left open at the end of a line goes on with the next line, blank lines included
    let mut quote = None;
    for (i, &(_, line)) in lines.iter().enumerate().skip(from) {
        // inline data that follows a command without a period ends that command
        let data = kind != Start::Comment && is_line_command(line, ["BEGIN", "DATA"]);
        if i > from && quote.is_none() && data {
            return (i, None, kind);
        }
        if line.trim().is_empty() {
            if quote.is_some() || kind == (Start::PerLine { document: true }) {
                continue;
            }
            return (i, None, kind);
        }
        let mut line_quote = None;
        let quote = match kind {
            Start::Code => &mut quote,
            _ => &mut line_quote,
        };
        let found = terminator(line, kind == Start::Comment, quote);
        // in free text a quote does not hide the period that ends the command
        if let Some(found) = found.filter(|_| kind != Start::Code || quote.is_none()) {
            return (i + 1, Some(found), kind);
        }
    }
    (lines.len(), None, kind)
}

/// Every command of the source, or the error that prevented tokenising it.
pub(crate) fn split(source: &str) -> Vec<Result<RawCommand, SyntaxError>> {
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    let lines: Vec<(u32, &str)> = (1u32..).zip(source.lines()).collect();
    let mut out = Vec::new();
    split_lines(&lines, &mut out);
    out
}

/// The commands of numbered lines.
fn split_lines(lines: &[(u32, &str)], out: &mut Vec<Result<RawCommand, SyntaxError>>) {
    let mut i = 0;
    while i < lines.len() {
        if lines[i].1.trim().is_empty() {
            i += 1;
            continue;
        }
        // inline data: the lines up to END DATA are data, whatever they look like
        if is_line_command(lines[i].1, ["BEGIN", "DATA"]) {
            let (command, next) = inline_data(lines, i);
            out.push(command);
            i = next;
            continue;
        }
        let (end, terminator, kind) = command_end(lines, i);
        let mut current = lines[i..end].to_vec();
        let last = current.len() - 1;
        let span = match terminator {
            Some((byte, column)) => {
                current[last].1 = &current[last].1[..byte];
                let line = current[last].0;
                Span {
                    line,
                    column,
                    end_line: line,
                    end_column: column + 1,
                }
            }
            None => {
                let (line, text) = current[last];
                let column = text.trim_end().chars().count() as u32 + 1;
                Span {
                    line,
                    column,
                    end_line: line,
                    end_column: column,
                }
            }
        };
        if kind != Start::Comment
            && let Some(command) = build(&current, span)
        {
            out.push(command);
        }
        i = end;
    }
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
