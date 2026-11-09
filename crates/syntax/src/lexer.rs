//! Tokens and the tokeniser.

use crate::error::{Span, SyntaxError};

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    /// A name or keyword, as written. Never ends with a period.
    Word(String),
    /// An unsigned number literal. A sign in front is a separate `Punct::Minus` token.
    Number(f64),
    /// A string literal with the quotes removed and doubled quotes collapsed.
    Str(String),
    Punct(Punct),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Punct {
    LParen,
    RParen,
    Comma,
    Slash,
    Eq,
    Plus,
    Minus,
    Star,
    /// `**`
    Pow,
    Lt,
    Gt,
    Le,
    Ge,
    /// `<>` or `~=`
    Ne,
    /// `&`
    And,
    /// `|`
    Or,
    /// `~`
    Not,
    LBracket,
    RBracket,
}

impl Punct {
    pub fn as_str(self) -> &'static str {
        match self {
            Punct::LParen => "(",
            Punct::RParen => ")",
            Punct::Comma => ",",
            Punct::Slash => "/",
            Punct::Eq => "=",
            Punct::Plus => "+",
            Punct::Minus => "-",
            Punct::Star => "*",
            Punct::Pow => "**",
            Punct::Lt => "<",
            Punct::Gt => ">",
            Punct::Le => "<=",
            Punct::Ge => ">=",
            Punct::Ne => "<>",
            Punct::And => "&",
            Punct::Or => "|",
            Punct::Not => "~",
            Punct::LBracket => "[",
            Punct::RBracket => "]",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

fn is_name_start(c: char) -> bool {
    c.is_alphabetic() || matches!(c, '@' | '#' | '$')
}

fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '.' | '@' | '#' | '$')
}

/// Tokenises the lines of one command. Each line comes with its 1-based line number and with
/// the command terminator already removed.
pub(crate) struct Lexer<'a> {
    lines: std::slice::Iter<'a, (u32, &'a str)>,
    line: u32,
    chars: Vec<char>,
    col: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(lines: &'a [(u32, &'a str)]) -> Self {
        Lexer {
            lines: lines.iter(),
            line: 0,
            chars: Vec::new(),
            col: 0,
        }
    }

    /// The next significant character, after skipping whitespace, inline comments and line ends.
    pub fn peek_char(&mut self) -> Option<char> {
        loop {
            while let Some(&c) = self.chars.get(self.col) {
                if c.is_whitespace() {
                    self.col += 1;
                } else if c == '/' && self.chars.get(self.col + 1) == Some(&'*') {
                    // an unclosed comment runs to the end of the line
                    let close = (self.col + 2..self.chars.len().saturating_sub(1))
                        .find(|&i| self.chars[i] == '*' && self.chars[i + 1] == '/');
                    self.col = close.map_or(self.chars.len(), |i| i + 2);
                } else {
                    return Some(c);
                }
            }
            let &(line, text) = self.lines.next()?;
            self.line = line;
            self.chars = text.chars().collect();
            self.col = 0;
        }
    }

    pub fn next_token(&mut self) -> Result<Option<Token>, SyntaxError> {
        let Some(c) = self.peek_char() else {
            return Ok(None);
        };
        let start = self.col;
        let next = self.chars.get(start + 1).copied();
        let kind = if c == '\'' || c == '"' {
            self.string(c)?
        } else if c.is_ascii_digit() || (c == '.' && next.is_some_and(|d| d.is_ascii_digit())) {
            self.number()?
        } else if is_name_start(c) {
            self.word()
        } else {
            self.punct(c, next)?
        };
        Ok(Some(Token {
            kind,
            span: self.span(start),
        }))
    }

    fn span(&self, start: usize) -> Span {
        Span {
            line: self.line,
            column: start as u32 + 1,
            end_line: self.line,
            end_column: self.col as u32 + 1,
        }
    }

    fn text(&self, start: usize) -> String {
        self.chars[start..self.col].iter().collect()
    }

    fn string(&mut self, quote: char) -> Result<TokenKind, SyntaxError> {
        let start = self.col;
        let mut text = String::new();
        self.col += 1;
        loop {
            match self.chars.get(self.col) {
                None => {
                    return Err(
                        SyntaxError::new(self.span(start), "unterminated string").hint(
                            "a string must close on the line where it starts; \
                         write a quote inside a string by doubling it",
                        ),
                    );
                }
                Some(&c) if c == quote => {
                    self.col += 1;
                    if self.chars.get(self.col) != Some(&quote) {
                        return Ok(TokenKind::Str(text));
                    }
                    text.push(quote);
                    self.col += 1;
                }
                Some(&c) => {
                    text.push(c);
                    self.col += 1;
                }
            }
        }
    }

    fn digits(&mut self) {
        while self.chars.get(self.col).is_some_and(char::is_ascii_digit) {
            self.col += 1;
        }
    }

    fn number(&mut self) -> Result<TokenKind, SyntaxError> {
        let start = self.col;
        self.digits();
        if self.chars.get(self.col) == Some(&'.') {
            self.col += 1;
            self.digits();
        }
        if matches!(self.chars.get(self.col), Some('e' | 'E')) {
            let sign = usize::from(matches!(self.chars.get(self.col + 1), Some('+' | '-')));
            let exponent = self.chars.get(self.col + 1 + sign);
            if exponent.is_some_and(char::is_ascii_digit) {
                self.col += 1 + sign;
                self.digits();
            }
        }
        let glued = |c: &char| is_name_start(*c) || *c == '_';
        if self.chars.get(self.col).is_some_and(glued) {
            while self.chars.get(self.col).is_some_and(|&c| is_name_char(c)) {
                self.col += 1;
            }
        } else if let Ok(value) = self.text(start).parse() {
            return Ok(TokenKind::Number(value));
        }
        let message = format!("invalid number {}", self.text(start));
        Err(SyntaxError::new(self.span(start), message))
    }

    fn word(&mut self) -> TokenKind {
        let start = self.col;
        while self.chars.get(self.col).is_some_and(|&c| is_name_char(c)) {
            self.col += 1;
        }
        // a name cannot end with a period; the first character is never one, so this stops
        while self.chars[self.col - 1] == '.' {
            self.col -= 1;
        }
        TokenKind::Word(self.text(start))
    }

    fn punct(&mut self, c: char, next: Option<char>) -> Result<TokenKind, SyntaxError> {
        let (punct, len) = match (c, next) {
            ('*', Some('*')) => (Punct::Pow, 2),
            ('<', Some('=')) => (Punct::Le, 2),
            ('<', Some('>')) => (Punct::Ne, 2),
            ('>', Some('=')) => (Punct::Ge, 2),
            ('~', Some('=')) => (Punct::Ne, 2),
            ('(', _) => (Punct::LParen, 1),
            (')', _) => (Punct::RParen, 1),
            (',', _) => (Punct::Comma, 1),
            ('/', _) => (Punct::Slash, 1),
            ('=', _) => (Punct::Eq, 1),
            ('+', _) => (Punct::Plus, 1),
            ('-', _) => (Punct::Minus, 1),
            ('*', _) => (Punct::Star, 1),
            ('<', _) => (Punct::Lt, 1),
            ('>', _) => (Punct::Gt, 1),
            ('&', _) => (Punct::And, 1),
            ('|', _) => (Punct::Or, 1),
            ('~', _) => (Punct::Not, 1),
            ('[', _) => (Punct::LBracket, 1),
            (']', _) => (Punct::RBracket, 1),
            _ => {
                let start = self.col;
                self.col += 1;
                let error =
                    SyntaxError::new(self.span(start), format!("unexpected character '{c}'"));
                return Err(match c {
                    '.' => error.hint(
                        "a period ends a command only as the last character of a line; \
                         start the next command on a new line",
                    ),
                    '!' => error.hint("not equal is written <> or ~=, NOT is written ~"),
                    _ => error,
                });
            }
        };
        self.col += len;
        Ok(TokenKind::Punct(punct))
    }
}
