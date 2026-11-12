//! Token cursor and the pieces shared by the command parsers.

use crate::ast::{Bound, MissingValue, Value, VarItem};
use crate::error::{Span, SyntaxError};
use crate::lexer::{Punct, Token, TokenKind};
use crate::table::matches_keyword;

pub(crate) type PResult<T> = Result<T, SyntaxError>;

const RESERVED: [&str; 13] = [
    "ALL", "AND", "BY", "EQ", "GE", "GT", "LE", "LT", "NE", "NOT", "OR", "TO", "WITH",
];

pub(crate) fn is_reserved(word: &str) -> bool {
    RESERVED.iter().any(|r| word.eq_ignore_ascii_case(r))
}

pub(crate) struct Parser<'a> {
    tokens: &'a [Token],
    pub pos: usize,
    end: Span,
    depth: u32,
    operators: u32,
    /// In a MATRIX subcommand a slash starts the next subcommand instead of dividing.
    pub slash_ends: bool,
}

impl<'a> Parser<'a> {
    /// `tokens` is the whole command, `body` the index of the first token after the command
    /// name (at least 1), `end` is where the command ends.
    pub fn new(tokens: &'a [Token], body: usize, end: Span) -> Self {
        Parser {
            tokens,
            pos: body,
            end,
            depth: 0,
            operators: 0,
            slash_ends: false,
        }
    }

    pub fn peek(&self) -> Option<&'a TokenKind> {
        self.peek_at(0)
    }

    pub fn peek_at(&self, offset: usize) -> Option<&'a TokenKind> {
        self.tokens.get(self.pos + offset).map(|t| &t.kind)
    }

    pub fn span(&self) -> Span {
        self.tokens.get(self.pos).map_or(self.end, |t| t.span)
    }

    /// The span from the token at `start` to the last consumed token.
    pub fn span_from(&self, start: usize) -> Span {
        match (self.tokens.get(start), self.pos.checked_sub(1)) {
            (Some(first), Some(last)) if last >= start => first.span.to(self.tokens[last].span),
            _ => self.span(),
        }
    }

    /// "unexpected X" at the current token, listing what would have been accepted.
    pub fn unexpected(&self, expected: &[&str]) -> SyntaxError {
        let found = match self.peek() {
            None => "end of command".to_string(),
            Some(TokenKind::Word(w)) => format!("word {w}"),
            Some(TokenKind::Number(n)) => format!("number {n}"),
            Some(TokenKind::Str(s)) => format!("string '{s}'"),
            Some(TokenKind::Punct(p)) => format!("'{}'", p.as_str()),
        };
        let mut error = SyntaxError::new(self.span(), format!("unexpected {found}"));
        error.expected = expected.iter().map(|e| e.to_string()).collect();
        error
    }

    /// Runs one level of a recursive rule. The limit turns runaway nesting into an error
    /// instead of a stack overflow, which would take the whole process down.
    pub fn nested<T>(&mut self, rule: impl FnOnce(&mut Self) -> PResult<T>) -> PResult<T> {
        // MATRIX expressions take about 20 KiB of stack per level in a debug build, and a run's
        // thread has 2 MiB
        if self.depth == 64 {
            let message = "expression is nested too deeply (more than 64 levels)";
            return Err(SyntaxError::new(self.span(), message));
        }
        self.depth += 1;
        let result = rule(self);
        self.depth -= 1;
        result
    }

    /// Counts one binary operator of an expression. Operator chains are parsed in a loop but
    /// build a tree as deep as the chain is long, and trees are dropped and walked
    /// recursively, so the length needs a limit just like the nesting does.
    pub fn count_operator(&mut self) -> PResult<()> {
        self.operators += 1;
        if self.operators > 5000 {
            let message = "expression is too long (more than 5000 operators)";
            return Err(SyntaxError::new(self.span(), message));
        }
        Ok(())
    }

    pub fn at(&self, punct: Punct) -> bool {
        self.peek() == Some(&TokenKind::Punct(punct))
    }

    pub fn eat(&mut self, punct: Punct) -> bool {
        let found = self.at(punct);
        self.pos += usize::from(found);
        found
    }

    pub fn expect(&mut self, punct: Punct) -> PResult<()> {
        if self.eat(punct) {
            Ok(())
        } else {
            Err(self.unexpected(&[&format!("'{}'", punct.as_str())]))
        }
    }

    /// A keyword that may be abbreviated to three or more letters.
    pub fn at_keyword(&self, keyword: &str) -> bool {
        matches!(self.peek(), Some(TokenKind::Word(w)) if matches_keyword(w, keyword))
    }

    pub fn eat_keyword(&mut self, keyword: &str) -> bool {
        let found = self.at_keyword(keyword);
        self.pos += usize::from(found);
        found
    }

    /// A word that must be written in full, such as the reserved words.
    pub fn eat_word(&mut self, word: &str) -> bool {
        let found = matches!(self.peek(), Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case(word));
        self.pos += usize::from(found);
        found
    }

    /// Succeeds at the end of the command. `also` names what else could have continued it.
    pub fn finish(&self, also: &[&str]) -> PResult<()> {
        if self.peek().is_none() {
            return Ok(());
        }
        let mut expected = also.to_vec();
        expected.push("end of command");
        let error = self.unexpected(&expected);
        // leftovers that begin a new line are most often the next command
        Err(if self.span().line > self.tokens[self.pos - 1].span.line {
            error.hint("if a new command starts here, end the previous command with a period")
        } else {
            error
        })
    }

    pub fn name(&mut self, what: &str) -> PResult<String> {
        match self.peek() {
            Some(TokenKind::Word(w)) if is_reserved(w) => {
                let message = format!(
                    "{} is a reserved word and cannot be used as a name",
                    w.to_uppercase()
                );
                let mut error = SyntaxError::new(self.span(), message);
                error.expected = vec![what.to_string()];
                Err(error)
            }
            Some(TokenKind::Word(w)) => {
                self.pos += 1;
                Ok(w.clone())
            }
            _ => Err(self.unexpected(&[what])),
        }
    }

    pub fn string(&mut self, what: &str) -> PResult<String> {
        let Some(TokenKind::Str(first)) = self.peek() else {
            return Err(self.unexpected(&[what]));
        };
        let mut text = first.clone();
        self.pos += 1;
        // a long string may be written as pieces joined with '+'
        while self.at(Punct::Plus)
            && let Some(TokenKind::Str(more)) = self.peek_at(1)
        {
            text.push_str(more);
            self.pos += 2;
        }
        Ok(text)
    }

    /// A file: a quoted path, or a file handle name (FILE HANDLE) without quotes.
    pub fn file(&mut self, what: &str) -> PResult<String> {
        match self.peek() {
            Some(TokenKind::Word(_)) => self.name(what),
            _ => self.string(what),
        }
    }

    /// A number literal, with an optional minus sign directly in front of it.
    pub fn number(&mut self) -> Option<f64> {
        match (self.peek(), self.peek_at(1)) {
            (Some(TokenKind::Number(n)), _) => {
                self.pos += 1;
                Some(*n)
            }
            (Some(TokenKind::Punct(Punct::Minus)), Some(TokenKind::Number(n))) => {
                self.pos += 2;
                Some(-n)
            }
            _ => None,
        }
    }

    pub fn value(&mut self) -> Option<Value> {
        if let Some(TokenKind::Str(s)) = self.peek() {
            self.pos += 1;
            return Some(Value::String(s.clone()));
        }
        self.number().map(Value::Number)
    }

    /// A value or a range such as `1 THRU 5`, `LOWEST THRU 0` or `90 THRU HIGHEST`.
    /// None when the next token starts neither.
    pub fn value_or_range(&mut self) -> PResult<Option<MissingValue>> {
        let low = if self.eat_word("LO") || self.eat_keyword("LOWEST") {
            Bound::Lowest
        } else {
            match self.value() {
                Some(Value::Number(n)) => Bound::Number(n),
                Some(text) => return Ok(Some(MissingValue::Value(text))),
                None => return Ok(None),
            }
        };
        if !self.eat_keyword("THRU") {
            return match low {
                Bound::Number(n) => Ok(Some(MissingValue::Value(Value::Number(n)))),
                _ => Err(self.unexpected(&["THRU"])),
            };
        }
        let high = if self.eat_word("HI") || self.eat_keyword("HIGHEST") {
            Bound::Highest
        } else if let Some(n) = self.number() {
            Bound::Number(n)
        } else {
            return Err(self.unexpected(&["a number", "HIGHEST"]));
        };
        Ok(Some(MissingValue::Range(low, high)))
    }

    /// One or more of: a name, `a TO b`, `ALL`. Commas between items are optional.
    pub fn var_list(&mut self) -> PResult<Vec<VarItem>> {
        let mut items = Vec::new();
        while let Some(TokenKind::Word(_)) = self.peek() {
            if self.eat_word("ALL") {
                items.push(VarItem::All);
            } else {
                let first = self.name("a variable name")?;
                if self.eat_word("TO") {
                    items.push(VarItem::Range(first, self.name("a variable name")?));
                } else {
                    items.push(VarItem::Name(first));
                }
            }
            self.eat(Punct::Comma);
        }
        if items.is_empty() {
            return Err(self.unexpected(&["a variable name"]));
        }
        Ok(items)
    }

    /// Error for a `/NAME` subcommand the command does not handle, with the parser just after
    /// the slash. `known` lists the real subcommands of the command that have no parser yet.
    pub fn subcommand_error(&self, command: &str, known: &[&str]) -> SyntaxError {
        let Some(TokenKind::Word(word)) = self.peek() else {
            return self.unexpected(&["a subcommand name"]);
        };
        let message = match known.iter().find(|k| matches_keyword(word, k)) {
            Some(name) => format!("subcommand {name} of {command} is not supported yet"),
            None => format!("unknown subcommand {} of {command}", word.to_uppercase()),
        };
        SyntaxError::new(self.span(), message)
    }
}
