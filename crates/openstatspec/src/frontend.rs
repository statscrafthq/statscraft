//! Syntax frontend profiles 0.1, 0.2 and 0.3: request in, plan or diagnostic out.
//!
//! The profiles are bounded on purpose, so this module has its own lexer and parser instead of
//! Statscraft's command language: it must reject much that Statscraft accepts, and its number
//! tokens have their own grammar.

use crate::plan::{
    Bits, Comparator, Connective, Level, Match, Op, Operand, Outcome, PLAN_V01, PLAN_V02, Plan,
    Predicate, Rule, TargetMode, Typed, ValueLabel,
};
use serde::Deserialize;
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
enum Version {
    V1,
    V2,
    V3,
}

impl Version {
    fn name(self) -> &'static str {
        match self {
            Version::V1 => "0.1",
            Version::V2 => "0.2",
            Version::V3 => "0.3",
        }
    }
}

/// The result of one frontend request.
#[derive(Debug)]
pub struct Output {
    /// SHA-256 of the LF-normalized source text; `None` only when the request has no source text.
    pub source_hash: Option<String>,
    pub result: Result<Compiled, Diagnostic>,
}

#[derive(Debug)]
pub struct Compiled {
    pub plan: Plan,
    /// Canonical JSON of the plan: the exact bytes of `plan_hash`.
    pub canonical: String,
    pub plan_hash: String,
}

/// A stable diagnostic code with a message and, for source errors, the position in the
/// LF-normalized source (byte offsets, 1-based line and column in characters).
#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
    pub code: &'static str,
    pub message: String,
    pub span: Option<Span>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}

/// CRLF and CR become LF, as the source hash requires.
pub fn normalize_source(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// Lowers one frontend request (contract, input alias, input schema, source text). On any
/// error no plan is produced.
pub fn compile_request(request: &Value) -> Output {
    let source = request
        .get("source_text")
        .and_then(Value::as_str)
        .map(normalize_source);
    let source_hash = source.as_deref().map(|s| crate::sha256_hex(s.as_bytes()));
    let result = parse_request(request).and_then(|(version, req)| {
        let source = source.expect("a valid request has source text");
        let compiled = compile(version, &req, &source).map(|plan| {
            let canonical = plan.canonical();
            Compiled {
                plan_hash: crate::sha256_hex(canonical.as_bytes()),
                canonical,
                plan,
            }
        });
        compiled.map_err(|e| {
            let line_start = source[..e.start].rfind('\n').map_or(0, |i| i + 1);
            Diagnostic {
                code: e.code,
                message: e.message,
                span: Some(Span {
                    start: e.start,
                    end: e.end,
                    line: source[..e.start].matches('\n').count() + 1,
                    column: source[line_start..e.start].chars().count() + 1,
                }),
            }
        })
    });
    Output {
        source_hash,
        result,
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    contract: String,
    input_alias: String,
    input_schema: InputSchema,
    source_text: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InputSchema {
    variables: Vec<InputVariable>,
}

#[derive(Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum StorageKind {
    Numeric,
    String,
}

// The labels, formats and level are validated but only value labels affect the lowering.
#[allow(dead_code)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InputVariable {
    name: String,
    storage_kind: StorageKind,
    #[serde(default)]
    variable_label: Option<String>,
    #[serde(default)]
    value_labels: Vec<ValueLabel>,
    #[serde(default)]
    format_family: Option<String>,
    #[serde(default)]
    width: Option<u64>,
    #[serde(default)]
    decimals: Option<u64>,
    #[serde(default)]
    measurement_level: Option<Level>,
}

fn parse_request(value: &Value) -> Result<(Version, Request), Diagnostic> {
    let invalid = |m: String| Diagnostic {
        code: "request_schema_invalid",
        message: m,
        span: None,
    };
    let req: Request = serde_json::from_value(value.clone()).map_err(|e| invalid(e.to_string()))?;
    let version = match crate::FRONTEND_CONTRACTS
        .iter()
        .position(|c| *c == req.contract)
    {
        Some(0) => Version::V1,
        Some(1) => Version::V2,
        Some(_) => Version::V3,
        None => {
            return Err(invalid(format!(
                "unknown request contract {}",
                req.contract
            )));
        }
    };
    let vars = &req.input_schema.variables;
    if req.input_alias.is_empty() || req.source_text.is_empty() || vars.is_empty() {
        return Err(invalid(
            "input_alias, source_text and input_schema.variables must not be empty".into(),
        ));
    }
    if vars.iter().any(|v| v.name.is_empty() || v.width == Some(0)) {
        return Err(invalid(
            "a variable needs a name and a width of at least 1".into(),
        ));
    }
    let v02_fields = ["format_family", "width", "decimals", "measurement_level"];
    let has_v02_field = value["input_schema"]["variables"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|v| v02_fields.iter().any(|f| v.get(*f).is_some()));
    if version == Version::V1 && has_v02_field {
        return Err(invalid(
            "format and measurement fields need request contract v0.2 or later".into(),
        ));
    }
    Ok((version, req))
}

/// An error before positions are turned into line and column.
struct E {
    code: &'static str,
    message: String,
    start: usize,
    end: usize,
}

type R<T> = Result<T, E>;

fn e<T>(code: &'static str, message: impl Into<String>, (start, end): (usize, usize)) -> R<T> {
    Err(E {
        code,
        message: message.into(),
        start,
        end,
    })
}

const SYNTAX: &str = crate::wire::SYNTAX_ERROR;
const UNSUPPORTED: &str = crate::wire::UNSUPPORTED_COMMAND;
const TYPE_UNSUPPORTED: &str = "expression_type_unsupported";

// ---------------------------------------------------------------------------------------------
// Lexer

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Ident(String),
    /// A finite-number token, already rounded to binary64; `signed` when it began with `-`.
    Num {
        value: f64,
        signed: bool,
    },
    Str(String),
    Punct(&'static str),
    /// The command terminator.
    Dot,
    /// A `*` or `COMMENT` command, including its closing period.
    Comment,
}

struct Token {
    tok: Tok,
    start: usize,
    end: usize,
}

fn ident_start(c: char) -> bool {
    c.is_alphabetic() || matches!(c, '_' | '@' | '#' | '$')
}

fn ident_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '@' | '#' | '$')
}

const PUNCTS: [&str; 18] = [
    "<=", ">=", "<>", "~=", "**", "(", ")", "=", ",", "/", "<", ">", "+", "-", "*", "~", "&", "|",
];

fn lex(src: &str, v: Version) -> R<Vec<Token>> {
    let at = |p: usize| src[p..].chars().next();
    let digit = |p: usize| at(p).is_some_and(|c| c.is_ascii_digit());
    let mut toks = Vec::new();
    let mut pos = 0;
    let mut at_command_start = true;
    loop {
        // whitespace and, from profile 0.3 on, block comments
        while let Some(c) = at(pos) {
            if c.is_whitespace() {
                pos += c.len_utf8();
            } else if src[pos..].starts_with("/*") {
                if v < Version::V3 {
                    return e(
                        SYNTAX,
                        "/* */ comments need frontend profile 0.3",
                        (pos, pos + 2),
                    );
                }
                let body = pos + 2;
                let Some(close) = src[body..].find("*/") else {
                    return e(SYNTAX, "unterminated /* comment", (pos, src.len()));
                };
                if let Some(open) = src[body..body + close].find("/*") {
                    let open = body + open;
                    return e(SYNTAX, "/* comments cannot be nested", (open, open + 2));
                }
                pos = body + close + 2;
            } else {
                break;
            }
        }
        let Some(c) = at(pos) else { break };
        let start = pos;

        let comment_word = src[pos..]
            .get(..7)
            .is_some_and(|w| w.eq_ignore_ascii_case("COMMENT"))
            && !at(pos + 7).is_some_and(ident_char);
        if at_command_start && (c == '*' || comment_word) {
            // A comment command ends at the first period followed by whitespace or the end.
            let end = src[pos..].char_indices().find_map(|(i, ch)| {
                let after = pos + i + 1;
                (ch == '.' && at(after).is_none_or(char::is_whitespace)).then_some(after)
            });
            let Some(end) = end else {
                return e(
                    SYNTAX,
                    "the comment has no closing period",
                    (pos, src.len()),
                );
            };
            toks.push(Token {
                tok: Tok::Comment,
                start,
                end,
            });
            pos = end;
            continue;
        }
        at_command_start = false;

        let tok = if c == '\'' || c == '"' {
            let mut text = String::new();
            pos += 1;
            loop {
                match at(pos) {
                    None => return e(SYNTAX, "unterminated string", (start, src.len())),
                    Some(ch) if ch == c && at(pos + 1) == Some(c) => {
                        text.push(c);
                        pos += 2;
                    }
                    Some(ch) if ch == c => {
                        pos += 1;
                        break;
                    }
                    Some(ch) => {
                        text.push(ch);
                        pos += ch.len_utf8();
                    }
                }
            }
            Tok::Str(text)
        } else if c.is_ascii_digit() || (c == '-' && digit(pos + 1)) {
            // finite-number := "-"? ("0" | [1-9][0-9]*) ("." [0-9]+)? ([eE] [+-]? [0-9]+)?
            if c == '-' {
                pos += 1;
            }
            let int_start = pos;
            while digit(pos) {
                pos += 1;
            }
            let int_digits = pos - int_start;
            let leading_zero = int_digits > 1 && src[int_start..].starts_with('0');
            if at(pos) == Some('.') && digit(pos + 1) {
                pos += 1;
                while digit(pos) {
                    pos += 1;
                }
            }
            if matches!(at(pos), Some('e' | 'E')) {
                let sign = usize::from(matches!(at(pos + 1), Some('+' | '-')));
                if digit(pos + 1 + sign) {
                    pos += 1 + sign;
                    while digit(pos) {
                        pos += 1;
                    }
                }
            }
            if leading_zero || at(pos).is_some_and(ident_char) {
                let end = pos + at(pos).map_or(0, char::len_utf8);
                return e(SYNTAX, "malformed number", (start, end));
            }
            // Rust's parser rounds the exact decimal value once, to nearest, ties to even.
            let value: f64 = src[start..pos].parse().expect("number grammar checked");
            if value.is_infinite() {
                return e(SYNTAX, "the number is too large for binary64", (start, pos));
            }
            Tok::Num {
                value: if value == 0.0 { 0.0 } else { value },
                signed: c == '-',
            }
        } else if ident_start(c) {
            pos += c.len_utf8();
            loop {
                match at(pos) {
                    Some(ch) if ident_char(ch) => pos += ch.len_utf8(),
                    // a period inside a name, as in F8.2; a trailing period ends the command
                    Some('.') if at(pos + 1).is_some_and(ident_char) => pos += 1,
                    _ => break,
                }
            }
            Tok::Ident(src[start..pos].to_string())
        } else if c == '.' {
            pos += 1;
            at_command_start = true;
            Tok::Dot
        } else if let Some(p) = PUNCTS.iter().find(|p| src[pos..].starts_with(**p)) {
            pos += p.len();
            Tok::Punct(p)
        } else {
            return e(
                SYNTAX,
                format!("unexpected character {c:?}"),
                (pos, pos + c.len_utf8()),
            );
        };
        toks.push(Token {
            tok,
            start,
            end: pos,
        });
    }
    Ok(toks)
}

// ---------------------------------------------------------------------------------------------
// Commands

/// The tokens of one command, without its period, and a cursor.
struct Cmd<'t> {
    toks: &'t [Token],
    i: usize,
    /// Position of the closing period, for errors at the end of the command.
    end: usize,
}

impl<'t> Cmd<'t> {
    fn peek(&self) -> Option<&'t Tok> {
        self.toks.get(self.i).map(|t| &t.tok)
    }

    fn span(&self) -> (usize, usize) {
        self.toks
            .get(self.i)
            .map_or((self.end, self.end + 1), |t| (t.start, t.end))
    }

    /// The whole command, for errors that belong to no single token.
    fn span_all(&self) -> (usize, usize) {
        (self.toks.first().map_or(self.end, |t| t.start), self.end)
    }

    fn bump(&mut self) -> Option<&'t Token> {
        let t = self.toks.get(self.i);
        self.i += 1;
        t
    }

    fn is_kw(&self, kw: &str) -> bool {
        matches!(self.peek(), Some(Tok::Ident(s)) if s.eq_ignore_ascii_case(kw))
    }

    fn eat_kw(&mut self, kw: &str) -> bool {
        let found = self.is_kw(kw);
        self.i += usize::from(found);
        found
    }

    fn is_punct(&self, p: &str) -> bool {
        matches!(self.peek(), Some(Tok::Punct(q)) if *q == p)
    }

    fn eat_punct(&mut self, p: &str) -> bool {
        let found = self.is_punct(p);
        self.i += usize::from(found);
        found
    }

    fn expect_punct(&mut self, p: &str) -> R<()> {
        if self.eat_punct(p) {
            Ok(())
        } else {
            e(SYNTAX, format!("expected {p}"), self.span())
        }
    }

    fn at_end(&self) -> bool {
        self.i >= self.toks.len()
    }

    fn expect_end(&self) -> R<()> {
        if self.at_end() {
            Ok(())
        } else {
            e(SYNTAX, "unexpected text before the period", self.span())
        }
    }

    fn string(&mut self) -> R<String> {
        match self.peek() {
            Some(Tok::Str(s)) => {
                self.i += 1;
                Ok(s.clone())
            }
            _ => e(SYNTAX, "expected a quoted string", self.span()),
        }
    }

    /// A number, or a quoted string: a typed value of a value label or recode.
    fn literal(&mut self) -> Option<Typed> {
        let typed = match self.peek()? {
            Tok::Num { value, .. } => number(*value),
            Tok::Str(s) => Typed::String { value: s.clone() },
            _ => return None,
        };
        self.i += 1;
        Some(typed)
    }

    fn number(&mut self) -> R<f64> {
        match self.peek() {
            Some(Tok::Num { value, .. }) => {
                self.i += 1;
                Ok(*value)
            }
            _ => e(SYNTAX, "expected a number", self.span()),
        }
    }
}

fn number(value: f64) -> Typed {
    Typed::Binary64 {
        bits: Bits(value.to_bits()),
    }
}

fn is_string(t: &Typed) -> bool {
    matches!(t, Typed::String { .. })
}

/// Words that can never name a variable.
fn reserved_word(name: &str) -> bool {
    [
        "ALL", "AND", "BY", "EQ", "GE", "GT", "LE", "LT", "NE", "NOT", "OR", "TO", "WITH",
    ]
    .iter()
    .any(|w| w.eq_ignore_ascii_case(name))
}

struct Var {
    name: String,
    numeric: bool,
    labels: Vec<ValueLabel>,
}

struct Compiler {
    v: Version,
    vars: Vec<Var>,
    ops: Vec<Op>,
}

/// A RECODE clause as written: its selectors and its result, each with a source span.
type Clause = (Vec<(Selector, (usize, usize))>, (Outcome, (usize, usize)));

/// One item of a RECODE value list.
enum Selector {
    Value(Typed),
    Range(f64, f64),
    SystemMissing,
}

/// A predicate as written, before NOT and NE are lowered.
enum Pred {
    Cmp(Operand, Option<Comparator>, Operand), // None is NE
    Bool(Connective, Vec<Pred>),
    Not(Box<Pred>),
}

fn compile(v: Version, req: &Request, source: &str) -> R<Plan> {
    let toks = lex(source, v)?;
    let mut c = Compiler {
        v,
        vars: req
            .input_schema
            .variables
            .iter()
            .map(|iv| Var {
                name: iv.name.clone(),
                numeric: iv.storage_kind == StorageKind::Numeric,
                labels: iv.value_labels.clone(),
            })
            .collect(),
        ops: Vec::new(),
    };
    let mut begin = 0;
    for (i, t) in toks.iter().enumerate() {
        match t.tok {
            Tok::Comment if v < Version::V3 => {
                return e(
                    UNSUPPORTED,
                    "comments need frontend profile 0.3",
                    (t.start, t.end),
                );
            }
            Tok::Comment => begin = i + 1,
            Tok::Dot if begin == i => return e(SYNTAX, "empty command", (t.start, t.end)),
            Tok::Dot => {
                c.command(Cmd {
                    toks: &toks[begin..i],
                    i: 0,
                    end: t.start,
                })?;
                begin = i + 1;
            }
            _ => {}
        }
    }
    if let Some(last) = toks.get(begin..).and_then(<[Token]>::last) {
        return e(
            SYNTAX,
            "the last command has no closing period",
            (last.start, last.end),
        );
    }
    if c.ops.is_empty() {
        return e(SYNTAX, "the program has no commands", (0, source.len()));
    }
    let contract = if c.ops.iter().any(Op::needs_v02) {
        PLAN_V02
    } else {
        PLAN_V01
    };
    Ok(Plan {
        contract: contract.into(),
        input_alias: req.input_alias.clone(),
        operations: c.ops,
    })
}

impl Compiler {
    fn command(&mut self, mut c: Cmd) -> R<()> {
        let first = c.bump().expect("commands are not empty");
        let Tok::Ident(name) = &first.tok else {
            return e(
                SYNTAX,
                "a command starts with its name",
                (first.start, first.end),
            );
        };
        let is = |w: &str| name.eq_ignore_ascii_case(w);
        let v2 = self.v >= Version::V2;
        let v3 = self.v >= Version::V3;
        if is("RECODE") {
            self.recode(c)
        } else if is("VARIABLE") && c.eat_kw("LABELS") {
            self.variable_labels(c)
        } else if is("VALUE") && c.eat_kw("LABELS") {
            self.value_labels(c, false)
        } else if v3 && is("ADD") && c.eat_kw("VALUE") && c.eat_kw("LABELS") {
            self.value_labels(c, true)
        } else if v2 && is("VARIABLE") && c.eat_kw("LEVEL") {
            self.variable_level(c)
        } else if v2 && is("COMPUTE") {
            self.compute(c)
        } else if v2 && is("IF") {
            self.if_command(c)
        } else if v2 && is("FORMATS") {
            self.formats(c)
        } else if v2 && is("EXECUTE") {
            c.expect_end()?;
            self.ops.push(Op::Execute {});
            Ok(())
        } else {
            e(
                UNSUPPORTED,
                format!(
                    "{name} is not supported by frontend profile {}",
                    self.v.name()
                ),
                (first.start, first.end),
            )
        }
    }

    /// Case-insensitive lookup that must yield at most one variable.
    fn lookup(&self, name: &str, span: (usize, usize)) -> R<Option<usize>> {
        let lower = name.to_lowercase();
        let mut hits = self
            .vars
            .iter()
            .enumerate()
            .filter(|(_, v)| v.name.to_lowercase() == lower)
            .map(|(i, _)| i);
        match (hits.next(), hits.next()) {
            (Some(_), Some(_)) => e(
                "unknown_variable",
                format!("{name} matches more than one variable"),
                span,
            ),
            (hit, _) => Ok(hit),
        }
    }

    /// The next token as a variable name, unresolved.
    fn name<'t>(&self, c: &mut Cmd<'t>) -> R<(&'t str, (usize, usize))> {
        let span = c.span();
        match c.peek() {
            Some(Tok::Ident(s)) if reserved_word(s) => {
                e(SYNTAX, format!("{s} is a reserved word"), span)
            }
            Some(Tok::Ident(s)) => {
                c.i += 1;
                Ok((s, span))
            }
            _ => e(SYNTAX, "expected a variable name", span),
        }
    }

    fn existing(&self, c: &mut Cmd) -> R<(usize, (usize, usize))> {
        let (name, span) = self.name(c)?;
        match self.lookup(name, span)? {
            Some(i) => Ok((i, span)),
            None => e("unknown_variable", format!("unknown variable {name}"), span),
        }
    }

    /// existing-variable ((existing-variable | TO existing-variable))*, TO from profile 0.3.
    fn var_list(&self, c: &mut Cmd) -> R<Vec<(usize, (usize, usize))>> {
        let mut out = vec![self.existing(c)?];
        while let Some(Tok::Ident(_)) = c.peek() {
            if self.v >= Version::V3 && c.is_kw("TO") {
                c.i += 1;
                let (last, from) = *out.last().expect("list starts with a variable");
                let (to, to_span) = self.existing(c)?;
                let span = (from.0, to_span.1);
                if to < last {
                    return e(
                        "invalid_variable_range",
                        format!(
                            "{} comes before {} in the dictionary",
                            self.vars[to].name, self.vars[last].name
                        ),
                        span,
                    );
                }
                out.extend((last + 1..=to).map(|i| (i, span)));
            } else {
                out.push(self.existing(c)?);
            }
        }
        Ok(out)
    }

    fn recode(&mut self, mut c: Cmd) -> R<()> {
        let sources = self.var_list(&mut c)?;
        if !c.is_punct("(") {
            return e(SYNTAX, "expected ( values = result )", c.span());
        }
        let mut rules: Vec<Clause> = vec![];
        let mut else_result = None;
        while c.is_punct("(") {
            let open = c.span();
            c.i += 1;
            if else_result.is_some() {
                return if c.is_kw("ELSE") {
                    e("duplicate_else", "ELSE appears twice", open)
                } else {
                    e("else_not_last", "ELSE must be the last clause", open)
                };
            }
            if c.eat_kw("ELSE") {
                c.expect_punct("=")?;
                else_result = Some(self.recode_result(&mut c)?);
            } else {
                let selectors = self.selectors(&mut c)?;
                c.expect_punct("=")?;
                rules.push((selectors, self.recode_result(&mut c)?));
            }
            c.expect_punct(")")?;
        }
        let mut targets = vec![];
        let into = c.eat_kw("INTO");
        if into {
            while let Some(Tok::Ident(_)) = c.peek() {
                if c.is_kw("TO") {
                    return e(SYNTAX, "TO cannot generate INTO targets", c.span());
                }
                targets.push(self.name(&mut c)?);
            }
            if targets.len() != sources.len() {
                return e(
                    SYNTAX,
                    format!(
                        "INTO names {} target(s) for {} source variable(s)",
                        targets.len(),
                        sources.len()
                    ),
                    c.span(),
                );
            }
        }
        c.expect_end()?;
        if rules.is_empty() {
            return e(SYNTAX, "RECODE needs a value clause besides ELSE", c.span());
        }

        // Type checks, source by source.
        let results = rules
            .iter()
            .map(|r| &r.1)
            .chain(else_result.as_ref())
            .collect::<Vec<_>>();
        for &(s, _) in &sources {
            let numeric = self.vars[s].numeric;
            let name = &self.vars[s].name;
            for (selector, span) in rules.iter().flat_map(|r| &r.0) {
                match selector {
                    Selector::Value(t) if is_string(t) == numeric => {
                        let kind = if numeric { "numeric" } else { "string" };
                        return e("type_mismatch", format!("{name} is {kind}"), *span);
                    }
                    Selector::Range(..) if !numeric => {
                        return e(
                            "type_mismatch",
                            format!("{name} is a string; ranges need a number"),
                            *span,
                        );
                    }
                    Selector::SystemMissing if !numeric => {
                        return e(
                            "system_missing_for_string",
                            format!("{name} is a string"),
                            *span,
                        );
                    }
                    _ => {}
                }
            }
            if into {
                // The created target takes the one storage kind the results determine.
                let mut kinds = results.iter().filter_map(|(o, _)| match o {
                    Outcome::Literal { value } => Some(is_string(value)),
                    Outcome::Copy {} => Some(!numeric),
                    Outcome::SystemMissing {} => None,
                });
                let first = kinds.next();
                if kinds.any(|k| Some(k) != first) {
                    return e(
                        "mixed_result_types",
                        "the results mix numbers and strings",
                        c.span_all(),
                    );
                }
                if first == Some(true) {
                    return e(
                        "string_target_requires_declaration",
                        "a string INTO target needs a declaration this profile does not have",
                        c.span_all(),
                    );
                }
            } else {
                for (outcome, span) in &results {
                    match outcome {
                        Outcome::Literal { value } if is_string(value) == numeric => {
                            return e(
                                "type_mismatch",
                                format!("the result does not fit {name}"),
                                *span,
                            );
                        }
                        Outcome::SystemMissing {} if !numeric => {
                            return e(
                                "system_missing_for_string",
                                format!("{name} is a string"),
                                *span,
                            );
                        }
                        _ => {}
                    }
                }
            }
        }
        for (k, (name, span)) in targets.iter().enumerate() {
            if name.starts_with("__") {
                return e(
                    "reserved_target_name",
                    "names beginning with __ are reserved",
                    *span,
                );
            }
            let earlier = targets[..k]
                .iter()
                .any(|(t, _)| t.to_lowercase() == name.to_lowercase());
            if earlier || self.lookup(name, *span)?.is_some() {
                return e(
                    "target_already_exists",
                    format!("{name} already exists"),
                    *span,
                );
            }
        }

        let rules: Vec<Rule> = rules
            .into_iter()
            .map(|(selectors, (result, _))| {
                let matches = match &selectors[..] {
                    [(Selector::Range(lo, hi), _)] => Match::Range {
                        lower: number(*lo),
                        upper: number(*hi),
                    },
                    [(Selector::SystemMissing, _)] => Match::SystemMissing {},
                    _ => Match::Values {
                        values: selectors
                            .into_iter()
                            .map(|(s, _)| match s {
                                Selector::Value(t) => t,
                                _ => unreachable!("checked when parsed"),
                            })
                            .collect(),
                    },
                };
                Rule { matches, result }
            })
            .collect();
        for (k, &(s, _)) in sources.iter().enumerate() {
            let (target, target_mode, default) = match targets.get(k) {
                Some((t, _)) => (t.to_string(), TargetMode::Create, Outcome::SystemMissing {}),
                None => (
                    self.vars[s].name.clone(),
                    TargetMode::Replace,
                    Outcome::Copy {},
                ),
            };
            self.ops.push(Op::Recode {
                source: self.vars[s].name.clone(),
                target,
                target_mode,
                rules: rules.clone(),
                unmatched: else_result.clone().map_or(default, |r| r.0),
            });
        }
        // Targets appear only after every pair has read the pre-command state.
        for (name, _) in targets {
            self.vars.push(Var {
                name: name.to_string(),
                numeric: true,
                labels: vec![],
            });
        }
        Ok(())
    }

    /// One clause's selector: a value list, a range, or SYSMIS.
    fn selectors(&self, c: &mut Cmd) -> R<Vec<(Selector, (usize, usize))>> {
        let v3 = self.v >= Version::V3;
        let mut items = vec![];
        loop {
            let start = c.span().0;
            // LOWEST and HIGHEST are the finite binary64 extremes (profile 0.3).
            let range_end = |c: &mut Cmd| {
                if v3 && c.eat_kw("HIGHEST") {
                    Ok(f64::MAX)
                } else if v3 && c.is_kw("LOWEST") {
                    e(SYNTAX, "LOWEST can only start a range", c.span())
                } else {
                    c.number()
                }
            };
            let item = if c.eat_kw("SYSMIS") {
                Selector::SystemMissing
            } else if v3 && c.is_kw("HIGHEST") {
                return e(SYNTAX, "HIGHEST can only end a range", c.span());
            } else if v3 && c.eat_kw("LOWEST") {
                if !c.eat_kw("THRU") {
                    return e(SYNTAX, "LOWEST must be followed by THRU", c.span());
                }
                Selector::Range(-f64::MAX, range_end(c)?)
            } else if let Some(Tok::Num { value, .. }) = c.peek() {
                c.i += 1;
                if c.eat_kw("THRU") {
                    Selector::Range(*value, range_end(c)?)
                } else {
                    Selector::Value(number(*value))
                }
            } else if let Some(t) = c.literal() {
                Selector::Value(t)
            } else {
                return e(SYNTAX, "expected a value, a range or SYSMIS", c.span());
            };
            let span = (start, c.toks[c.i - 1].end);
            if let Selector::Range(lo, hi) = item
                && lo > hi
            {
                // Profile 0.3 names invalid_variable_range for reverse ranges; earlier profiles
                // name no code, so the plan's own diagnostic is used.
                let code = if v3 {
                    "invalid_variable_range"
                } else {
                    "invalid_numeric_range"
                };
                return e(code, "the range starts above its end", span);
            }
            items.push((item, span));
            if !c.eat_punct(",") {
                break;
            }
        }
        if items.len() > 1 && items.iter().any(|(s, _)| !matches!(s, Selector::Value(_))) {
            return e(
                SYNTAX,
                "a value list cannot contain ranges or SYSMIS; write them as separate clauses",
                items[0].1,
            );
        }
        Ok(items)
    }

    fn recode_result(&self, c: &mut Cmd) -> R<(Outcome, (usize, usize))> {
        let span = c.span();
        let outcome = if c.eat_kw("SYSMIS") {
            Outcome::SystemMissing {}
        } else if c.eat_kw("COPY") {
            Outcome::Copy {}
        } else if let Some(value) = c.literal() {
            Outcome::Literal { value }
        } else {
            return e(SYNTAX, "expected a value, SYSMIS or COPY", span);
        };
        Ok((outcome, span))
    }

    fn variable_labels(&mut self, mut c: Cmd) -> R<()> {
        loop {
            let (i, _) = self.existing(&mut c)?;
            let label = c.string()?;
            self.ops.push(Op::SetVariableLabel {
                variable: self.vars[i].name.clone(),
                label,
            });
            if c.at_end() {
                return Ok(());
            }
        }
    }

    /// VALUE LABELS replaces the label set; ADD VALUE LABELS merges into the current one.
    fn value_labels(&mut self, mut c: Cmd, add: bool) -> R<()> {
        loop {
            let targets = self.var_list(&mut c)?;
            let mut pairs: Vec<ValueLabel> = vec![];
            loop {
                let span = c.span();
                let Some(value) = c.literal() else { break };
                let label = c.string()?;
                if let Some((i, _)) = targets
                    .iter()
                    .find(|(i, _)| is_string(&value) == self.vars[*i].numeric)
                {
                    let name = &self.vars[*i].name;
                    return e(
                        "type_mismatch",
                        format!("the value does not fit {name}"),
                        span,
                    );
                }
                if pairs.iter().any(|p| p.value == value) {
                    return e(
                        "duplicate_value_label",
                        "the value already has a label here",
                        span,
                    );
                }
                pairs.push(ValueLabel { value, label });
            }
            if pairs.is_empty() {
                return e(SYNTAX, "expected a value and its label", c.span());
            }
            for (i, _) in targets {
                let var = &mut self.vars[i];
                let labels = &mut var.labels;
                if add {
                    for pair in &pairs {
                        match labels.iter_mut().find(|l| l.value == pair.value) {
                            Some(existing) => existing.label = pair.label.clone(),
                            None => labels.push(pair.clone()),
                        }
                    }
                } else {
                    *labels = pairs.clone();
                }
                self.ops.push(Op::ReplaceValueLabels {
                    variable: var.name.clone(),
                    labels: labels.clone(),
                });
            }
            if !c.eat_punct("/") {
                return c.expect_end();
            }
        }
    }

    fn variable_level(&mut self, mut c: Cmd) -> R<()> {
        loop {
            let targets = self.var_list(&mut c)?;
            c.expect_punct("(")?;
            let level = if c.eat_kw("NOMINAL") {
                Level::Nominal
            } else if c.eat_kw("ORDINAL") {
                Level::Ordinal
            } else if c.eat_kw("SCALE") {
                Level::Scale
            } else {
                return e(SYNTAX, "expected NOMINAL, ORDINAL or SCALE", c.span());
            };
            c.expect_punct(")")?;
            for (i, _) in targets {
                self.ops.push(Op::SetMeasurementLevel {
                    variable: self.vars[i].name.clone(),
                    level,
                });
            }
            if !c.eat_punct("/") {
                return c.expect_end();
            }
        }
    }

    fn formats(&mut self, mut c: Cmd) -> R<()> {
        loop {
            // Profile 0.2 takes one variable per format, 0.3 a variable list.
            let targets = if self.v >= Version::V3 {
                self.var_list(&mut c)?
            } else {
                vec![self.existing(&mut c)?]
            };
            for &(i, span) in &targets {
                self.numeric(i, span, "FORMATS")?;
            }
            c.expect_punct("(")?;
            let span = c.span();
            let Some(Tok::Ident(spec)) = c.peek() else {
                return e(SYNTAX, "expected a format such as F8.2", span);
            };
            c.i += 1;
            let (width, decimals) = parse_f_format(spec).map_err(|m| E {
                code: "invalid_format",
                message: format!("{spec}: {m}"),
                start: span.0,
                end: span.1,
            })?;
            c.expect_punct(")")?;
            for (i, _) in targets {
                self.ops.push(Op::SetFormat {
                    variable: self.vars[i].name.clone(),
                    family: "F".into(),
                    width,
                    decimals,
                });
            }
            if c.at_end() {
                return Ok(());
            }
        }
    }

    fn numeric(&self, i: usize, span: (usize, usize), what: &str) -> R<()> {
        if self.vars[i].numeric {
            Ok(())
        } else {
            let name = &self.vars[i].name;
            e(
                TYPE_UNSUPPORTED,
                format!("{what} needs a numeric variable; {name} is a string"),
                span,
            )
        }
    }

    fn compute(&mut self, mut c: Cmd) -> R<()> {
        let (name, span) = self.name(&mut c)?;
        let existing = self.lookup(name, span)?;
        match existing {
            Some(i) => self.numeric(i, span, "COMPUTE")?,
            None if name.starts_with("__") => {
                return e(
                    "reserved_target_name",
                    "names beginning with __ are reserved",
                    span,
                );
            }
            None => {}
        }
        c.expect_punct("=")?;
        let value = self.operand(&mut c)?;
        c.expect_end()?;
        let (target, target_mode) = match existing {
            Some(i) => (self.vars[i].name.clone(), TargetMode::Replace),
            None => {
                self.vars.push(Var {
                    name: name.to_string(),
                    numeric: true,
                    labels: vec![],
                });
                (name.to_string(), TargetMode::Create)
            }
        };
        self.ops.push(Op::Assign {
            target,
            target_mode,
            value,
        });
        Ok(())
    }

    fn if_command(&mut self, mut c: Cmd) -> R<()> {
        if !c.eat_punct("(") {
            return e(SYNTAX, "IF needs its condition in parentheses", c.span());
        }
        let predicate = self.or_expr(&mut c)?;
        c.expect_punct(")")?;
        let (name, span) = self.name(&mut c)?;
        let Some(i) = self.lookup(name, span)? else {
            return e(
                "conditional_target_missing",
                format!("{name} does not exist; IF changes only an existing variable"),
                span,
            );
        };
        self.numeric(i, span, "IF")?;
        c.expect_punct("=")?;
        let value = self.operand(&mut c)?;
        c.expect_end()?;
        self.ops.push(Op::ConditionalAssign {
            condition: lower(predicate, false),
            target: self.vars[i].name.clone(),
            value,
        });
        Ok(())
    }

    /// operand := numeric-variable | finite-number
    fn operand(&self, c: &mut Cmd) -> R<Operand> {
        let span = c.span();
        let operand = match c.peek() {
            Some(Tok::Num { value, .. }) => {
                c.i += 1;
                Operand::Literal {
                    value: number(*value),
                }
            }
            Some(Tok::Ident(s))
                if s.eq_ignore_ascii_case("NaN") || s.eq_ignore_ascii_case("Infinity") =>
            {
                return e(SYNTAX, format!("{s} is not a finite number"), span);
            }
            Some(Tok::Ident(s))
                if !reserved_word(s)
                    && matches!(c.toks.get(c.i + 1), Some(t) if t.tok == Tok::Punct("(")) =>
            {
                return e(TYPE_UNSUPPORTED, "functions are not supported", span);
            }
            Some(Tok::Ident(_)) => {
                let (i, span) = self.existing(c)?;
                self.numeric(i, span, "an expression")?;
                Operand::Variable {
                    variable: self.vars[i].name.clone(),
                }
            }
            Some(Tok::Str(_)) => {
                return e(
                    TYPE_UNSUPPORTED,
                    "strings are not supported in expressions",
                    span,
                );
            }
            Some(Tok::Punct("-")) => {
                return e(TYPE_UNSUPPORTED, "unary minus is not supported", span);
            }
            _ => return e(SYNTAX, "expected a variable or a number", span),
        };
        if matches!(
            c.peek(),
            Some(Tok::Punct("+" | "-" | "*" | "/" | "**") | Tok::Num { signed: true, .. })
        ) {
            return e(TYPE_UNSUPPORTED, "arithmetic is not supported", c.span());
        }
        Ok(operand)
    }

    fn or_expr(&self, c: &mut Cmd) -> R<Pred> {
        self.chain(c, Connective::Or)
    }

    /// A chain of one connective: OR over AND chains, AND over NOT terms.
    fn chain(&self, c: &mut Cmd, op: Connective) -> R<Pred> {
        let word = match op {
            Connective::Or => "OR",
            Connective::And => "AND",
        };
        let term = |c: &mut Cmd| match op {
            Connective::Or => self.chain(c, Connective::And),
            Connective::And => self.not_term(c),
        };
        let mut terms = vec![term(c)?];
        while c.eat_kw(word) {
            terms.push(term(c)?);
        }
        Ok(if terms.len() == 1 {
            terms.pop().expect("one term")
        } else {
            Pred::Bool(op, terms)
        })
    }

    fn not_term(&self, c: &mut Cmd) -> R<Pred> {
        if self.v >= Version::V3 && c.eat_kw("NOT") {
            return Ok(Pred::Not(Box::new(self.not_term(c)?)));
        }
        if c.eat_punct("(") {
            let inner = self.or_expr(c)?;
            c.expect_punct(")")?;
            return Ok(inner);
        }
        let left = self.operand(c)?;
        let v3 = self.v >= Version::V3;
        let op = match c.peek() {
            Some(Tok::Punct("=")) => Some(Comparator::Eq),
            Some(Tok::Punct("<")) => Some(Comparator::Lt),
            Some(Tok::Punct("<=")) => Some(Comparator::Le),
            Some(Tok::Punct(">")) => Some(Comparator::Gt),
            Some(Tok::Punct(">=")) => Some(Comparator::Ge),
            Some(Tok::Punct("<>" | "~=")) if v3 => None,
            Some(Tok::Ident(s)) if v3 && s.eq_ignore_ascii_case("NE") => None,
            None | Some(Tok::Punct(")")) => {
                return e(
                    TYPE_UNSUPPORTED,
                    "a number is not a condition; compare it",
                    c.span(),
                );
            }
            Some(Tok::Ident(s))
                if s.eq_ignore_ascii_case("AND") || s.eq_ignore_ascii_case("OR") =>
            {
                return e(
                    TYPE_UNSUPPORTED,
                    "a number is not a condition; compare it",
                    c.span(),
                );
            }
            _ => return e(SYNTAX, "expected a comparison operator", c.span()),
        };
        c.i += 1;
        let right = self.operand(c)?;
        Ok(Pred::Cmp(left, op, right))
    }
}

/// Fwidth.decimals with a width of 1-40 and decimals 0-16 that leave room for a sign, the
/// decimal point and the decimals.
fn parse_f_format(spec: &str) -> Result<(u64, u64), &'static str> {
    let family_len = spec
        .find(|c: char| !c.is_ascii_alphabetic())
        .unwrap_or(spec.len());
    let (family, rest) = spec.split_at(family_len);
    if !family.eq_ignore_ascii_case("F") {
        return Err("only the F format family is supported");
    }
    let int = |s: &str| {
        (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            .then(|| s.parse::<u64>().ok())
            .flatten()
    };
    let (w, d) = rest
        .split_once('.')
        .and_then(|(w, d)| Some((int(w)?, int(d)?)))
        .ok_or("write the format as Fwidth.decimals")?;
    if !(1..=40).contains(&w) || d > 16 || (d > 0 && w < d + 2) {
        return Err("width must be 1-40, decimals 0-16, and width at least decimals + 2");
    }
    Ok((w, d))
}

/// Lowers NOT and NE with comparison complements and De Morgan's laws, and flattens chains of
/// one connective, keeping source order.
fn lower(p: Pred, negate: bool) -> Predicate {
    use Comparator::*;
    let node = |op: Connective, children: Vec<Predicate>| {
        let mut operands = vec![];
        for child in children {
            match child {
                Predicate::Boolean {
                    operator,
                    operands: inner,
                } if operator == op => operands.extend(inner),
                other => operands.push(other),
            }
        }
        Predicate::Boolean {
            operator: op,
            operands,
        }
    };
    match p {
        Pred::Not(inner) => lower(*inner, !negate),
        Pred::Bool(op, terms) => {
            let op = match (op, negate) {
                (Connective::And, false) | (Connective::Or, true) => Connective::And,
                _ => Connective::Or,
            };
            node(op, terms.into_iter().map(|t| lower(t, negate)).collect())
        }
        Pred::Cmp(left, op, right) => {
            let cmp = |operator| Predicate::Comparison {
                left: left.clone(),
                operator,
                right: right.clone(),
            };
            match (op, negate) {
                (Some(Eq), true) | (None, false) => node(Connective::Or, vec![cmp(Lt), cmp(Gt)]),
                (None, true) => node(Connective::And, vec![cmp(Ge), cmp(Le)]),
                (Some(op), false) => cmp(op),
                (Some(op), true) => cmp(match op {
                    Lt => Ge,
                    Le => Gt,
                    Gt => Le,
                    Ge => Lt,
                    Eq => unreachable!("handled above"),
                }),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn run(version: &str, source: &str) -> Result<Compiled, Diagnostic> {
        let profile = ["0.1", "0.2", "0.3"].iter().position(|v| *v == version);
        compile_request(&json!({
            "contract": crate::FRONTEND_CONTRACTS[profile.unwrap()],
            "input_alias": "d",
            "input_schema": {"variables": [
                {"name": "Age", "storage_kind": "numeric"},
                {"name": "city", "storage_kind": "string"}]},
            "source_text": source}))
        .result
    }

    fn code(version: &str, source: &str) -> &'static str {
        run(version, source).unwrap_err().code
    }

    fn bits(c: &Compiled, op: usize) -> Value {
        c.plan.to_json()["operations"][op]["value"]["value"]["bits"].clone()
    }

    #[test]
    fn diagnostics_beyond_the_published_cases() {
        assert_eq!(
            code("0.1", "RECODE age (1=0) (ELSE=1) (ELSE=2)."),
            "duplicate_else"
        );
        assert_eq!(
            code("0.1", "RECODE age (1=0) INTO CITY."),
            "target_already_exists"
        );
        assert_eq!(
            code("0.1", "RECODE age (1=0) (2='x') INTO new."),
            "mixed_result_types"
        );
        assert_eq!(code("0.1", "RECODE age (1, 2 THRU 3=0)."), SYNTAX);
        assert_eq!(code("0.1", "RECODE age (1=0)"), SYNTAX);
        assert_eq!(code("0.1", "COMPUTE x = 1."), UNSUPPORTED);
        assert_eq!(
            code("0.2", "RECODE age (5 THRU 3=0)."),
            "invalid_numeric_range"
        );
        assert_eq!(
            code("0.3", "RECODE age (5 THRU 3=0)."),
            "invalid_variable_range"
        );
        assert_eq!(code("0.2", "IF (age NE 1) age = 1."), SYNTAX);
        assert_eq!(code("0.3", "COMPUTE __x = 1."), "reserved_target_name");
        assert_eq!(
            code("0.3", "COMPUTE x = abs(age)."),
            "expression_type_unsupported"
        );
        assert_eq!(code("0.3", "COMPUTE x = 1 /* a /* b */."), SYNTAX);
    }

    #[test]
    fn lowering_and_positions() {
        // A star comment ends at a period followed by whitespace, so 1.5 inside it is text.
        let c = run("0.3", "* keep 1.5 here.\nIF (NOT (age NE 1)) age = 1.").unwrap();
        let cmp = |op: &str| {
            json!({"expression": "comparison", "left": {"kind": "variable", "variable": "Age"},
                   "operator": op,
                   "right": {"kind": "literal", "value": {"type": "binary64", "bits": "3ff0000000000000"}}})
        };
        assert_eq!(
            c.plan.to_json()["operations"][0]["condition"],
            json!({"expression": "boolean", "operator": "and", "operands": [cmp(">="), cmp("<=")]})
        );

        // 2^53 + 1 is a tie and rounds to even; negative zero becomes positive zero.
        let c = run(
            "0.2",
            "COMPUTE x = 9007199254740993.\nCOMPUTE x = -0.\nCOMPUTE x = 0.1.",
        )
        .unwrap();
        assert_eq!(bits(&c, 0), "4340000000000000");
        assert_eq!(bits(&c, 1), "0000000000000000");
        assert_eq!(bits(&c, 2), "3fb999999999999a");
        assert_eq!(c.plan.contract, PLAN_V02);

        let span = run("0.3", "EXECUTE.\r\nCOMPUTE x = 'a'.")
            .unwrap_err()
            .span
            .unwrap();
        assert_eq!((span.line, span.column, span.start), (2, 13, 21));
    }
}
