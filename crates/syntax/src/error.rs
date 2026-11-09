//! Source positions and the syntax error type.

use std::fmt;

/// A region of the source. Lines and columns are 1-based and columns count characters, not
/// bytes. The end is exclusive: `end_column` is the column just after the last character, so a
/// zero-width span (for example "end of command") has `column == end_column`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub line: u32,
    pub column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

impl Span {
    pub(crate) fn to(self, end: Span) -> Span {
        Span {
            end_line: end.end_line,
            end_column: end.end_column,
            ..self
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxError {
    pub message: String,
    pub span: Span,
    /// What would have been accepted at `span`, when the parser knows.
    pub expected: Vec<String>,
    pub hint: Option<String>,
}

impl SyntaxError {
    pub(crate) fn new(span: Span, message: impl Into<String>) -> Self {
        SyntaxError {
            message: message.into(),
            span,
            expected: Vec::new(),
            hint: None,
        }
    }

    pub(crate) fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

impl fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "line {}, column {}: {}",
            self.span.line, self.span.column, self.message
        )?;
        if !self.expected.is_empty() {
            write!(f, "; expected: {}", self.expected.join(", "))?;
        }
        if let Some(hint) = &self.hint {
            write!(f, "; hint: {hint}")?;
        }
        Ok(())
    }
}

impl std::error::Error for SyntaxError {}
