//! Splitting source text into commands and selecting commands by line range.

use crate::error::{Span, SyntaxError};
use crate::lexer::{Lexer, Token, TokenKind};
use crate::preprocess::{Line, MacroState, Piece, preprocess_with};
use crate::table::{Resolution, matches_keyword, resolve_tokens};
use std::path::Path;

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
    /// Where the text comes from when it is not the source line itself: a macro expansion or
    /// an inserted file, such as "the expansion of macro !TABLES".
    pub origin: Option<String>,
    /// With SET MPRINT=YES: the command as macro expansion made it.
    pub echo: Option<String>,
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
pub(crate) enum Start {
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
        // a quoted FILE LABEL, TITLE or SUBTITLE follows the rules for strings
        Resolution::Supported(spec, used)
            if FREE_TEXT.contains(&spec.name)
                && !(matches!(spec.name, "FILE LABEL" | "TITLE" | "SUBTITLE")
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
            lexer.glued_words = matches!(
                spec.name,
                "DATA LIST" | "PRINT" | "PRINT EJECT" | "WRITE" | "REPEATING DATA"
            );
            lexer.colon = matches!(spec.name, "NOMREG" | "PLUM" | "GENLIN" | "CSLOGISTIC");
            // only a command with a parser has a body worth tokenising
            while lex_error.is_none() {
                match lexer.next_token() {
                    Ok(Some(token)) => tokens.push(token),
                    Ok(None) => break,
                    Err(error) => lex_error = Some(error),
                }
            }
            if let Some(error) = lex_error {
                return Some(Err(multiline_hint(&tokens, error)));
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
        origin: None,
        echo: None,
    }))
}

/// A statement of a MATRIX program: every token, with the braces, colons and elementwise
/// operators of the matrix language, named by the matrix statement it starts.
fn build_matrix(lines: &[(u32, &str)], end: Span) -> Option<Result<RawCommand, SyntaxError>> {
    let mut lexer = Lexer::new(lines);
    lexer.matrix = true;
    lexer.glued_words = true;
    let mut tokens = Vec::new();
    loop {
        match lexer.next_token() {
            Ok(Some(token)) => tokens.push(token),
            Ok(None) => break,
            Err(error) => return Some(Err(multiline_hint(&tokens, error))),
        }
    }
    let name = match crate::matrix::statement_name(&tokens) {
        Some((name, _)) => name.to_string(),
        None => match &tokens.first()?.kind {
            TokenKind::Word(w) => w.to_uppercase(),
            _ => String::new(),
        },
    };
    Some(Ok(RawCommand {
        name,
        first_line: tokens[0].span.line,
        last_line: lines[lines.len() - 1].0,
        tokens,
        end,
        origin: None,
        echo: None,
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

/// The blocks whose lines are taken as they are: inline data, and a chart as JSON (a Statscraft
/// extension).
const BLOCKS: [&str; 2] = ["DATA", "CHART"];

/// The block that starts on `line`, by its second word: BEGIN DATA or BEGIN CHART.
pub(crate) fn block_start(line: &str) -> Option<&'static str> {
    BLOCKS
        .into_iter()
        .find(|&word| is_line_command(line, ["BEGIN", word]))
}

/// The BEGIN DATA (or BEGIN CHART) command on line index `first` with its lines, up to the
/// END DATA (END CHART) line, and the index of the line after that. Each line becomes a string
/// token on its own line, so the lines keep their numbers.
fn inline_data(
    lines: &[(u32, &str)],
    first: usize,
    block: &str,
) -> (Result<RawCommand, SyntaxError>, usize) {
    let line_span = |(number, text): (u32, &str)| Span {
        line: number,
        column: 1,
        end_line: number,
        end_column: text.chars().count() as u32 + 1,
    };
    let Some(end) = (first + 1..lines.len()).find(|&i| is_line_command(lines[i].1, ["END", block]))
    else {
        let message = format!("BEGIN {block} without END {block}");
        let hint = format!("end the block with a line that holds only END {block}");
        let error = SyntaxError::new(line_span(lines[first]), message).hint(hint);
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
    let mut tokens = vec![word("BEGIN"), word(block)];
    for &(line, text) in &lines[first + 1..end] {
        tokens.push(Token {
            kind: TokenKind::Str(text.to_string()),
            span: line_span((line, text)),
        });
    }
    let command = RawCommand {
        name: format!("BEGIN {block}"),
        first_line: number,
        last_line: lines[end].0,
        tokens,
        end: line_span(lines[end]),
        origin: None,
        echo: None,
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
        let data = kind != Start::Comment && block_start(line).is_some();
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

/// An error after a string that runs over several lines gets a hint: a closing quote left out
/// makes the string run on and the error show up later.
pub(crate) fn multiline_hint(tokens: &[Token], error: SyntaxError) -> SyntaxError {
    let before = |t: &&Token| (t.span.line, t.span.column) < (error.span.line, error.span.column);
    let long = tokens
        .iter()
        .filter(before)
        .find(|t| matches!(t.kind, TokenKind::Str(_)) && t.span.end_line > t.span.line);
    match long {
        Some(t) => {
            let line = t.span.line;
            error.hint(format!(
                "the string that starts on line {line} runs over several lines; \
                 if it was meant to end on line {line}, its closing quote is missing"
            ))
        }
        None => error,
    }
}

/// Every command of the source, or the error that prevented tokenising it. Macros are
/// expanded and files inserted first; `base` is the folder relative paths start from.
pub(crate) fn split(source: &str, base: Option<&Path>) -> Vec<Result<RawCommand, SyntaxError>> {
    split_with(source, base, &mut MacroState::default(), None)
}

/// `split` starting from the macros of `macros`, which afterwards also holds those the lines
/// of `range` define (see `preprocess_with`).
fn split_with(
    source: &str,
    base: Option<&Path>,
    macros: &mut MacroState,
    range: Option<(u32, u32)>,
) -> Vec<Result<RawCommand, SyntaxError>> {
    let mut out = Vec::new();
    let mut run: Vec<&Line> = Vec::new();
    let mut in_matrix = false;
    let pieces = preprocess_with(source, base, macros, range);
    for piece in &pieces {
        match piece {
            Piece::Line(line) => run.push(line),
            Piece::Error(error) => {
                split_lines(&run, &mut out, &mut in_matrix);
                run.clear();
                out.push(Err(error.clone()));
            }
        }
    }
    split_lines(&run, &mut out, &mut in_matrix);
    // nothing after FINISH is read; with a range only a FINISH inside it ends the run
    let finish = out.iter().position(|c| {
        matches!(c, Ok(c) if c.name == "FINISH"
            && range.is_none_or(|(first, last)| (first..=last).contains(&c.first_line)))
    });
    if let Some(at) = finish {
        out.truncate(at + 1);
    }
    out
}

/// A line that starts a command with `+`, `-` or `.` and a blank before its name, as batch
/// syntax allows for indenting: the line with that character blanked out.
fn unmarked(line: &str) -> Option<String> {
    let rest = line.trim_start();
    let mut chars = rest.chars();
    let marker = chars.next().filter(|c| matches!(c, '+' | '-' | '.'))?;
    let after = chars.as_str();
    let name = after.trim_start();
    if after.len() == name.len() || !name.starts_with(|c: char| c.is_ascii_alphabetic()) {
        return None;
    }
    let at = line.len() - rest.len();
    Some(format!(
        "{}{}{}",
        &line[..at],
        " ".repeat(marker.len_utf8()),
        after
    ))
}

/// The commands of numbered lines.
fn split_lines(
    run: &[&Line],
    out: &mut Vec<Result<RawCommand, SyntaxError>>,
    in_matrix: &mut bool,
) {
    let marked: Vec<Option<String>> = run.iter().map(|l| unmarked(&l.text)).collect();
    let mut lines: Vec<(u32, &str)> = run.iter().map(|l| (l.number, l.text.as_str())).collect();
    let mut i = 0;
    while i < lines.len() {
        if lines[i].1.trim().is_empty() {
            i += 1;
            continue;
        }
        // inline data: the lines up to END DATA are data, whatever they look like
        if let Some(block) = block_start(lines[i].1) {
            let (command, next) = inline_data(&lines, i, block);
            out.push(command);
            i = next;
            continue;
        }
        if let Some(text) = &marked[i] {
            lines[i].1 = text;
        }
        let (end, terminator, kind) = command_end(&lines, i);
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
        let built = match kind {
            Start::Comment => None,
            _ if *in_matrix => build_matrix(&current, span),
            _ => build(&current, span),
        };
        // the statements of a MATRIX program are read up to END MATRIX
        match &built {
            Some(Ok(command)) if command.name == "MATRIX" => *in_matrix = true,
            Some(Ok(command)) if command.name == "END MATRIX" => *in_matrix = false,
            _ => {}
        }
        if let Some(command) = built {
            let origin = run[i].origin.clone();
            let echo = run[i..end].iter().find_map(|l| l.echo.clone());
            out.push(match command {
                Ok(command) => Ok(RawCommand {
                    origin: origin.clone(),
                    echo,
                    ..command
                }),
                Err(error) => Err(match origin {
                    Some(origin) if error.hint.is_none() => error.hint(format!("in {origin}")),
                    _ => error,
                }),
            });
        }
        i = end;
    }
}

/// Splits the source into commands with their exact line ranges. Comment commands and empty
/// commands are left out. Fails on the first command that cannot be tokenised (for example an
/// unterminated string); an unknown command name is not an error here but in `parse_command`.
/// Macros are expanded and files inserted first, relative to the current folder.
pub fn split_commands(source: &str) -> Result<Vec<RawCommand>, SyntaxError> {
    split(source, None).into_iter().collect()
}

/// `split_commands` for syntax whose relative file names start from `base`, the folder of
/// the syntax file.
pub fn split_commands_in(
    source: &str,
    base: Option<&Path>,
) -> Result<Vec<RawCommand>, SyntaxError> {
    split(source, base).into_iter().collect()
}

/// `split_commands_in` for a session: macro calls see the macros of `macros` too, and
/// afterwards `macros` also holds those defined on the lines of `range` (all lines when None),
/// with the macro settings those lines changed.
pub fn split_commands_with(
    source: &str,
    base: Option<&Path>,
    macros: &mut MacroState,
    range: Option<(u32, u32)>,
) -> Result<Vec<RawCommand>, SyntaxError> {
    split_with(source, base, macros, range)
        .into_iter()
        // an error on a line outside the range does not stop the lines that run
        .filter(|c| match (c, range) {
            (Err(e), Some((first, last))) => (first..=last).contains(&e.span.line),
            _ => true,
        })
        .collect()
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
