//! The macro facility: DEFINE ... !ENDDEFINE and macro calls, expanded before commands are
//! parsed.
//!
//! Rules as implemented here:
//!
//! - A macro is defined by `DEFINE name (arguments) body !ENDDEFINE.` and called by its name,
//!   which is matched without regard to case and may start with `!`. A call may stand anywhere
//!   in a command except in quoted strings and comments.
//! - Arguments are positional (`!POSITIONAL`, referred to as `!1`, `!2`, all of them as `!*`)
//!   and then keyword arguments (`name = ...`, referred to as `!name`, given as `name = value`
//!   in any order). Each takes `!TOKENS(n)`, `!CHAREND('c')`, `!ENCLOSE('a', 'b')` or
//!   `!CMDEND`; an argument that the call leaves out gets its `!DEFAULT(...)` or nothing. The
//!   end of the calling command ends every argument. Tokens of the call that no argument takes
//!   stay in the command after the expansion.
//! - Outside quotes the body is read in upper case. In the body `!LET`, `!IF ... !THEN ...
//!   [!ELSE ...] !IFEND`, `!DO !var = a !TO b [!BY c] ... !DOEND`, `!DO !var !IN (list) ...
//!   !DOEND`, `!BREAK`, `!OFFEXPAND`/`!ONEXPAND` and the functions `!LENGTH`, `!CONCAT`,
//!   `!SUBSTR`, `!INDEX`, `!HEAD`, `!TAIL`, `!QUOTE`, `!UNQUOTE`, `!UPCASE`, `!BLANKS`,
//!   `!NULL` and `!EVAL` work as documented. Function arguments are not scanned for macro
//!   calls unless `!EVAL` asks for it.
//! - Expressions compare their unquoted operands, as numbers when both are numbers and else as
//!   text, case sensitive; `0` is false and anything else true.
//! - The expansion is scanned again for macro calls, up to 50 levels deep. A loop stops with an
//!   error after 1000 rounds.
//! - A body that ends with a period ends the calling command when the call is the last thing
//!   in it. In the middle of a command that last period is dropped, so a macro that holds a
//!   title (`'text'.`) can stand inside a subcommand.

use crate::table::matches_keyword;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    /// A name, keyword or other identifier.
    Id,
    /// A macro identifier: `!` followed by a name or digits, or `!*`.
    Bang,
    /// A quoted string; `text` keeps the quotes.
    Str,
    Num,
    Punct,
    Newline,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Tok {
    pub kind: Kind,
    pub text: String,
    /// Whitespace came before the token.
    pub space: bool,
    /// 0-based line of the tokenised text and 1-based column of the first character.
    pub line: u32,
    pub column: u32,
}

impl Tok {
    pub fn is(&self, text: &str) -> bool {
        self.kind != Kind::Str && self.text.eq_ignore_ascii_case(text)
    }

    /// A macro keyword such as `!TOKENS`, also shortened to three or more letters.
    fn keyword(&self, keyword: &str) -> bool {
        self.kind == Kind::Bang && matches_keyword(&self.text[1..], keyword)
    }

    fn upper(&self) -> String {
        self.text.to_uppercase()
    }
}

fn is_name_start(c: char) -> bool {
    c.is_alphabetic() || matches!(c, '@' | '#' | '$' | '_')
}

fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '.' | '@' | '#' | '$')
}

/// Splits text into macro tokens. Never fails: an unclosed string runs to the end of the text,
/// and the command parser reports it later.
pub(crate) fn tokenize(text: &str, newlines: bool) -> Vec<Tok> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let (mut i, mut line, mut line_start) = (0, 0u32, 0);
    let mut space = false;
    while i < chars.len() {
        let c = chars[i];
        let column = (i - line_start) as u32 + 1;
        let start = i;
        let kind = if c == '\n' {
            i += 1;
            line += 1;
            line_start = i;
            if newlines {
                out.push(Tok {
                    kind: Kind::Newline,
                    text: "\n".into(),
                    space: false,
                    line: line - 1,
                    column,
                });
            }
            space = true;
            continue;
        } else if c.is_whitespace() {
            i += 1;
            space = true;
            continue;
        } else if c == '/' && chars.get(i + 1) == Some(&'*') {
            let close = (i + 2..chars.len().saturating_sub(1))
                .find(|&k| chars[k] == '\n' || (chars[k] == '*' && chars[k + 1] == '/'));
            i = match close {
                Some(k) if chars[k] == '\n' => k,
                Some(k) => k + 2,
                None => chars.len(),
            };
            space = true;
            continue;
        } else if c == '\'' || c == '"' {
            i += 1;
            while i < chars.len() {
                if chars[i] == c {
                    if chars.get(i + 1) == Some(&c) {
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                if chars[i] == '\n' {
                    line += 1;
                    line_start = i + 1;
                }
                i += 1;
            }
            Kind::Str
        } else if c.is_ascii_digit()
            || (c == '.' && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit()))
        {
            while chars.get(i).is_some_and(char::is_ascii_digit) {
                i += 1;
            }
            if chars.get(i) == Some(&'.') && chars.get(i + 1).is_some_and(char::is_ascii_digit) {
                i += 1;
            }
            while chars.get(i).is_some_and(char::is_ascii_digit) {
                i += 1;
            }
            Kind::Num
        } else if is_name_start(c) {
            while chars.get(i).is_some_and(|&c| is_name_char(c)) {
                i += 1;
            }
            while chars[i - 1] == '.' {
                i -= 1;
            }
            Kind::Id
        } else if c == '!' && chars.get(i + 1) == Some(&'*') {
            i += 2;
            Kind::Bang
        } else if c == '!'
            && chars
                .get(i + 1)
                .is_some_and(|&c| is_name_char(c) && c != '.')
        {
            i += 1;
            while chars.get(i).is_some_and(|&c| is_name_char(c)) {
                i += 1;
            }
            while chars[i - 1] == '.' {
                i -= 1;
            }
            Kind::Bang
        } else {
            let two: String = chars[i..chars.len().min(i + 2)].iter().collect();
            i += if ["**", "<=", ">=", "<>", "~=", "¬="].contains(&two.as_str()) {
                2
            } else {
                1
            };
            Kind::Punct
        };
        out.push(Tok {
            kind,
            text: chars[start..i].iter().collect(),
            space,
            line,
            column,
        });
        space = false;
    }
    out
}

/// Tokens as text, one space where the source had whitespace.
pub(crate) fn render(tokens: &[Tok]) -> String {
    let mut out = String::new();
    for (i, t) in tokens.iter().enumerate() {
        if t.kind == Kind::Newline {
            out.push('\n');
            continue;
        }
        if t.space && i > 0 && !out.ends_with('\n') {
            out.push(' ');
        }
        out.push_str(&t.text);
    }
    out
}

/// The contents of a quoted string with doubled quotes made single; other text unchanged.
pub(crate) fn unquote(text: &str) -> String {
    let tokens = tokenize(text.trim(), false);
    match tokens.as_slice() {
        [t] if t.kind == Kind::Str && t.text.len() >= 2 => {
            let quote = &t.text[..1];
            let inner = &t.text[1..t.text.len() - 1];
            inner.replace(&format!("{quote}{quote}"), quote)
        }
        _ => text.to_string(),
    }
}

fn quote(text: &str) -> String {
    let tokens = tokenize(text.trim(), false);
    match tokens.as_slice() {
        [t] if t.kind == Kind::Str => text.trim().to_string(),
        _ => format!("'{}'", text.replace('\'', "''")),
    }
}

#[derive(Debug, Clone, PartialEq)]
enum ArgKind {
    Tokens(usize),
    CharEnd(String),
    Enclose(String, String),
    CmdEnd,
}

#[derive(Debug, Clone)]
struct Param {
    /// "1", "2", ... for positional arguments, else the keyword in upper case.
    name: String,
    positional: bool,
    default: String,
    kind: ArgKind,
}

#[derive(Debug, Clone)]
pub(crate) struct Macro {
    pub name: String,
    params: Vec<Param>,
    body: Vec<Tok>,
}

/// An error with the token it is about, if any.
pub(crate) type MResult<T> = Result<T, (Option<(u32, u32)>, String)>;

fn fail<T>(token: Option<&Tok>, message: impl Into<String>) -> MResult<T> {
    Err((token.map(|t| (t.line, t.column)), message.into()))
}

/// Reads `DEFINE name (arguments) body !ENDDEFINE` from its tokens, newlines included and the
/// `!ENDDEFINE` token last.
pub(crate) fn define(tokens: &[Tok]) -> MResult<Macro> {
    let code: Vec<usize> = (0..tokens.len())
        .filter(|&i| tokens[i].kind != Kind::Newline)
        .collect();
    let at = |k: usize| code.get(k).map(|&i| &tokens[i]);
    let Some(name) = at(1).filter(|t| matches!(t.kind, Kind::Id | Kind::Bang)) else {
        return fail(at(1).or(at(0)), "DEFINE needs a macro name");
    };
    if at(2).is_none_or(|t| !t.is("(")) {
        return fail(at(2).or(Some(name)), "the macro name must be followed by (");
    }
    let mut k = 3;
    let mut params: Vec<Param> = Vec::new();
    let positional_count = |params: &[Param]| params.iter().filter(|p| p.positional).count();
    while at(k).is_some_and(|t| !t.is(")")) {
        let first = at(k).expect("checked");
        let (name, positional) = if first.keyword("POSITIONAL") {
            k += 1;
            if params.iter().any(|p| !p.positional) {
                return fail(
                    Some(first),
                    "positional arguments must come before keyword ones",
                );
            }
            ((positional_count(&params) + 1).to_string(), true)
        } else if first.kind == Kind::Id && at(k + 1).is_some_and(|t| t.is("=")) {
            k += 2;
            (first.upper(), false)
        } else {
            return fail(
                Some(first),
                "expected !POSITIONAL or an argument name and =",
            );
        };
        let (mut default, mut kind) = (String::new(), None);
        loop {
            let Some(t) = at(k) else {
                return fail(Some(first), "the argument list is not closed with )");
            };
            if t.is("/") || t.is(")") {
                break;
            }
            let open = |k: usize| -> MResult<()> {
                match at(k) {
                    Some(p) if p.is("(") => Ok(()),
                    other => fail(other.or(Some(t)), format!("{} needs (", t.text)),
                }
            };
            let close = |k: usize| -> MResult<()> {
                match at(k) {
                    Some(p) if p.is(")") => Ok(()),
                    other => fail(other.or(Some(t)), format!("{} needs )", t.text)),
                }
            };
            let string = |k: usize| -> MResult<String> {
                match at(k) {
                    Some(s) if s.kind == Kind::Str => Ok(unquote(&s.text)),
                    other => fail(
                        other.or(Some(t)),
                        format!("{} needs a quoted character", t.text),
                    ),
                }
            };
            if t.keyword("DEFAULT") {
                open(k + 1)?;
                let (mut depth, mut end) = (0, k + 2);
                while let Some(d) = at(end) {
                    if d.is(")") && depth == 0 {
                        break;
                    }
                    depth += i32::from(d.is("(")) - i32::from(d.is(")"));
                    end += 1;
                }
                close(end)?;
                let slice: Vec<Tok> = (k + 2..end)
                    .map(|j| at(j).expect("in range").clone())
                    .collect();
                default = render(&slice);
                k = end + 1;
                continue;
            }
            if t.keyword("NOEXPAND") {
                // shortcut: arguments are always scanned for macro calls; honour !NOEXPAND when a file needs it
                k += 1;
                continue;
            }
            let found = if t.keyword("TOKENS") {
                open(k + 1)?;
                let n = at(k + 2)
                    .filter(|n| n.kind == Kind::Num)
                    .and_then(|n| n.text.parse::<usize>().ok())
                    .filter(|&n| n > 0);
                let Some(n) = n else {
                    return fail(at(k + 2), "!TOKENS needs a positive whole number");
                };
                close(k + 3)?;
                k += 4;
                ArgKind::Tokens(n)
            } else if t.keyword("CHAREND") {
                open(k + 1)?;
                let c = string(k + 2)?;
                close(k + 3)?;
                k += 4;
                ArgKind::CharEnd(c)
            } else if t.keyword("ENCLOSE") {
                open(k + 1)?;
                let a = string(k + 2)?;
                if at(k + 3).is_none_or(|t| !t.is(",")) {
                    return fail(
                        at(k + 3),
                        "!ENCLOSE needs two quoted characters separated by a comma",
                    );
                }
                let b = string(k + 4)?;
                close(k + 5)?;
                k += 6;
                ArgKind::Enclose(a, b)
            } else if t.keyword("CMDEND") {
                k += 1;
                ArgKind::CmdEnd
            } else {
                return fail(
                    Some(t),
                    format!("unexpected {} in the argument definition", t.text),
                );
            };
            if kind.replace(found).is_some() {
                return fail(
                    Some(t),
                    "an argument takes only one of !TOKENS, !CHAREND, !ENCLOSE and !CMDEND",
                );
            }
        }
        let Some(kind) = kind else {
            return fail(
                Some(first),
                "the argument needs !TOKENS, !CHAREND, !ENCLOSE or !CMDEND",
            );
        };
        params.push(Param {
            name,
            positional,
            default,
            kind,
        });
        if at(k).is_some_and(|t| t.is("/")) {
            k += 1;
        }
    }
    if at(k).is_none() {
        return fail(Some(name), "the argument list is not closed with )");
    }
    // the body starts after ")", and after a period that ends the DEFINE line
    let mut body_start = code[k] + 1;
    if tokens.get(body_start).is_some_and(|t| t.is("."))
        && tokens
            .get(body_start + 1)
            .is_none_or(|t| t.kind == Kind::Newline)
    {
        body_start += 1;
    }
    let body_end = tokens.len() - 1;
    let mut body: Vec<Tok> = tokens[body_start.min(body_end)..body_end].to_vec();
    while body.first().is_some_and(|t| t.kind == Kind::Newline) {
        body.remove(0);
    }
    while body.last().is_some_and(|t| t.kind == Kind::Newline) {
        body.pop();
    }
    // the body is read in upper case, except quoted strings
    for t in &mut body {
        if matches!(t.kind, Kind::Id | Kind::Bang) {
            t.text = t.text.to_uppercase();
        }
    }
    Ok(Macro {
        name: name.upper(),
        params,
        body,
    })
}

/// The defined macros, by upper-case name, and the limits of their expansion.
#[derive(Clone, Debug)]
pub(crate) struct Macros {
    pub defined: HashMap<String, Macro>,
    /// SET MNEST: how deep macro calls may nest.
    pub nest: usize,
    /// SET MITERATE: how many rounds a !DO loop may run.
    pub iterate: usize,
}

impl Default for Macros {
    fn default() -> Self {
        Self {
            defined: HashMap::new(),
            nest: 50,
            iterate: 1000,
        }
    }
}

enum Flow {
    Normal,
    Break,
}

struct Env<'a> {
    args: &'a HashMap<String, String>,
    vars: HashMap<String, String>,
}

/// Where a command ends inside `tokens` from `from`: at a period that ends a line, else at the
/// end of the tokens.
fn command_end(tokens: &[Tok], from: usize) -> usize {
    (from..tokens.len())
        .find(|&j| tokens[j].is(".") && tokens.get(j + 1).is_none_or(|t| t.kind == Kind::Newline))
        .unwrap_or(tokens.len())
}

fn push_value(out: &mut String, space: bool, value: &str) {
    if value.is_empty() {
        return;
    }
    if space && !out.is_empty() && !out.ends_with('\n') && !out.ends_with(' ') {
        out.push(' ');
    }
    out.push_str(value);
}

fn format_number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{n:.0}")
    } else {
        n.to_string()
    }
}

impl Macros {
    pub fn is_macro(&self, token: &Tok) -> bool {
        matches!(token.kind, Kind::Id | Kind::Bang) && self.defined.contains_key(&token.upper())
    }

    /// Expands every macro call in `tokens`. `Ok(None)` when there is none.
    pub fn expand(&self, tokens: &[Tok], depth: usize) -> MResult<Option<String>> {
        let directive = |t: &Tok| t.is("!OFFEXPAND") || t.is("!ONEXPAND");
        if !tokens.iter().any(|t| self.is_macro(t) || directive(t)) {
            return Ok(None);
        }
        let mut out = String::new();
        let mut expanding = true;
        let mut i = 0;
        while i < tokens.len() {
            let t = &tokens[i];
            if t.is("!OFFEXPAND") || t.is("!ONEXPAND") {
                expanding = t.is("!ONEXPAND");
                i += 1;
                continue;
            }
            if !(expanding && self.is_macro(t)) {
                match t.kind {
                    Kind::Newline => out.push('\n'),
                    _ => push_value(&mut out, t.space, &t.text),
                }
                i += 1;
                continue;
            }
            if depth >= self.nest {
                return fail(
                    Some(t),
                    format!("macro calls are nested more than {} levels deep", self.nest),
                );
            }
            let definition = &self.defined[&t.upper()];
            let end = command_end(tokens, i + 1);
            let (args, next) = self.arguments(definition, tokens, i + 1, end)?;
            let text = self
                .call(definition, &args, depth)
                .map_err(|(_, message)| {
                    (
                        Some((t.line, t.column)),
                        format!("in macro {}: {message}", definition.name),
                    )
                })?;
            let rest = tokens[next..end].iter().any(|t| t.kind != Kind::Newline);
            let mut text = collapse(&text);
            if rest && text.ends_with('.') {
                text.pop();
            }
            // at the outermost level the lines of an expansion are marked apart from the
            // line breaks of the source, so that they keep the number of the call's line
            let mark = if depth == 0 { "\r" } else { "\n" };
            push_value(&mut out, t.space, &text.trim_end().replace('\n', mark));
            if text.ends_with('.') && next < tokens.len() {
                // the period of the body ends the command; what follows starts a new line
                out.push_str(mark);
            }
            i = next;
        }
        Ok(Some(out))
    }

    /// The values of the arguments of a call whose tokens after the name start at `from`, and
    /// the index of the first token no argument took.
    fn arguments(
        &self,
        definition: &Macro,
        tokens: &[Tok],
        from: usize,
        end: usize,
    ) -> MResult<(HashMap<String, String>, usize)> {
        let code: Vec<usize> = (from..end)
            .filter(|&i| tokens[i].kind != Kind::Newline)
            .collect();
        let mut k = 0;
        let mut values = HashMap::new();
        let take = |k: &mut usize, kind: &ArgKind| -> Option<String> {
            let start = *k;
            let value = |a: usize, b: usize| {
                let slice: Vec<Tok> = code[a..b].iter().map(|&i| tokens[i].clone()).collect();
                render(&slice).trim().to_string()
            };
            match kind {
                ArgKind::Tokens(n) => {
                    *k = (start + n).min(code.len());
                    (*k > start).then(|| value(start, *k))
                }
                ArgKind::CharEnd(c) => {
                    let stop = (start..code.len()).find(|&j| tokens[code[j]].text == *c);
                    *k = stop.map_or(code.len(), |j| j + 1);
                    (*k > start).then(|| value(start, stop.unwrap_or(code.len())))
                }
                ArgKind::Enclose(a, b) => {
                    if code.get(start).is_none_or(|&i| tokens[i].text != *a) {
                        return None;
                    }
                    let stop = (start + 1..code.len()).find(|&j| tokens[code[j]].text == *b);
                    *k = stop.map_or(code.len(), |j| j + 1);
                    Some(value(start + 1, stop.unwrap_or(code.len())))
                }
                ArgKind::CmdEnd => {
                    *k = code.len();
                    (*k > start).then(|| value(start, code.len()))
                }
            }
        };
        for param in definition.params.iter().filter(|p| p.positional) {
            if let Some(value) = take(&mut k, &param.kind) {
                values.insert(format!("!{}", param.name), value);
            }
        }
        while let Some(&i) = code.get(k) {
            let keyword = definition.params.iter().find(|p| {
                !p.positional && tokens[i].kind == Kind::Id && tokens[i].upper() == p.name
            });
            let Some(param) =
                keyword.filter(|_| code.get(k + 1).is_some_and(|&j| tokens[j].is("=")))
            else {
                break;
            };
            k += 2;
            let value = take(&mut k, &param.kind).unwrap_or_default();
            values.insert(format!("!{}", param.name), value);
        }
        for param in &definition.params {
            values
                .entry(format!("!{}", param.name))
                .or_insert_with(|| param.default.clone());
        }
        let positional: Vec<String> = (1..)
            .map_while(|n| values.get(&format!("!{n}")).cloned())
            .filter(|v| !v.is_empty())
            .collect();
        values.insert("!*".into(), positional.join(" "));
        // just after the last token taken, so line breaks after the call stay
        let next = if k == 0 { from } else { code[k - 1] + 1 };
        Ok((values, next))
    }

    /// The expansion of one call, scanned again for macro calls.
    fn call(
        &self,
        definition: &Macro,
        args: &HashMap<String, String>,
        depth: usize,
    ) -> MResult<String> {
        let mut env = Env {
            args,
            vars: HashMap::new(),
        };
        let mut out = String::new();
        self.run(&definition.body, &mut env, &mut out)?;
        let tokens = tokenize(&out, true);
        Ok(self.expand(&tokens, depth + 1)?.unwrap_or(out))
    }

    /// Interprets body tokens into `out`.
    fn run(&self, tokens: &[Tok], env: &mut Env, out: &mut String) -> MResult<Flow> {
        let mut i = 0;
        while i < tokens.len() {
            let t = &tokens[i];
            if t.kind == Kind::Newline {
                out.push('\n');
                i += 1;
                continue;
            }
            if t.kind != Kind::Bang {
                push_value(out, t.space, &t.text);
                i += 1;
                continue;
            }
            match t.text.as_str() {
                "!LET" => {
                    let var = tokens.get(i + 1).filter(|v| v.kind == Kind::Bang);
                    let Some(var) = var else {
                        return fail(Some(t), "!LET needs a macro variable such as !x");
                    };
                    if env.args.contains_key(&var.text) {
                        return fail(
                            Some(var),
                            format!("!LET cannot change the argument {}", var.text),
                        );
                    }
                    if tokens.get(i + 2).is_none_or(|e| !e.is("=")) {
                        return fail(
                            tokens.get(i + 2).or(Some(var)),
                            "!LET needs = after the variable",
                        );
                    }
                    let mut k = i + 3;
                    let value = self.expression(tokens, &mut k, env)?;
                    env.vars.insert(var.text.clone(), value);
                    i = k;
                }
                "!IF" => {
                    let mut k = i + 1;
                    let condition = self.expression(tokens, &mut k, env)?;
                    if tokens.get(k).is_none_or(|t| !t.is("!THEN")) {
                        return fail(
                            tokens.get(k).or(Some(t)),
                            "!IF needs !THEN after its condition",
                        );
                    }
                    let (otherwise, end) = matching(tokens, k + 1, "!IF", "!IFEND", Some("!ELSE"))
                        .ok_or_else(|| {
                            (Some((t.line, t.column)), "!IF without !IFEND".to_string())
                        })?;
                    let branch = if truth(&condition) {
                        &tokens[k + 1..otherwise.unwrap_or(end)]
                    } else {
                        otherwise.map_or(&tokens[end..end], |e| &tokens[e + 1..end])
                    };
                    if let Flow::Break = self.run(branch, env, out)? {
                        return Ok(Flow::Break);
                    }
                    i = end + 1;
                }
                "!DO" => {
                    let var = tokens.get(i + 1).filter(|v| v.kind == Kind::Bang);
                    let Some(var) = var else {
                        return fail(Some(t), "!DO needs a loop variable such as !i");
                    };
                    let mut k = i + 2;
                    let items: Vec<String> = if tokens.get(k).is_some_and(|e| e.is("=")) {
                        k += 1;
                        let number = |text: String, at: &Tok| {
                            unquote(&text).trim().parse::<f64>().map_err(|_| {
                                (
                                    Some((at.line, at.column)),
                                    format!("!DO needs a number, got {text}"),
                                )
                            })
                        };
                        let first = number(self.operand(tokens, &mut k, env)?, t)?;
                        if tokens.get(k).is_none_or(|e| !e.is("!TO")) {
                            return fail(tokens.get(k).or(Some(t)), "!DO needs !TO");
                        }
                        k += 1;
                        let last = number(self.operand(tokens, &mut k, env)?, t)?;
                        let step = if tokens.get(k).is_some_and(|e| e.is("!BY")) {
                            k += 1;
                            number(self.operand(tokens, &mut k, env)?, t)?
                        } else {
                            1.0
                        };
                        if step == 0.0 {
                            return fail(Some(t), "!BY must not be 0");
                        }
                        // the number of rounds after the first; negative when there are none
                        let after = ((last - first) / step).floor();
                        if after >= self.iterate as f64 {
                            return fail(
                                Some(t),
                                format!("the loop would run more than {} times", self.iterate),
                            );
                        }
                        let rounds = if after < 0.0 { 0 } else { after as usize + 1 };
                        (0..rounds)
                            .map(|n| format_number(first + n as f64 * step))
                            .collect()
                    } else if tokens.get(k).is_some_and(|e| e.is("!IN")) {
                        k += 1;
                        let list = self.expression(tokens, &mut k, env)?;
                        let list = tokenize(&unquote(&list), false);
                        list.into_iter().map(|t| t.text).collect()
                    } else {
                        return fail(
                            tokens.get(k).or(Some(t)),
                            "!DO needs = or !IN after the variable",
                        );
                    };
                    let (_, end) = matching(tokens, k, "!DO", "!DOEND", None).ok_or_else(|| {
                        (Some((t.line, t.column)), "!DO without !DOEND".to_string())
                    })?;
                    if items.len() > self.iterate {
                        return fail(
                            Some(t),
                            format!("the loop would run more than {} times", self.iterate),
                        );
                    }
                    for item in items {
                        env.vars.insert(var.text.clone(), item);
                        if let Flow::Break = self.run(&tokens[k..end], env, out)? {
                            break;
                        }
                    }
                    i = end + 1;
                }
                "!BREAK" => return Ok(Flow::Break),
                "!THEN" | "!ELSE" | "!IFEND" | "!DOEND" | "!TO" | "!BY" | "!IN" => {
                    return fail(Some(t), format!("{} without its !IF or !DO", t.text));
                }
                "!OFFEXPAND" | "!ONEXPAND" => {
                    push_value(out, true, &t.text);
                    i += 1;
                }
                _ => {
                    let space = t.space;
                    let mut k = i;
                    let value = self.operand(tokens, &mut k, env)?;
                    push_value(out, space, &value);
                    i = k;
                }
            }
        }
        Ok(Flow::Normal)
    }

    /// `or` level of a macro expression; the result of a comparison or logical operator is 1
    /// or 0, otherwise the operand's value.
    fn expression(&self, tokens: &[Tok], k: &mut usize, env: &mut Env) -> MResult<String> {
        let mut left = self.and(tokens, k, env)?;
        while tokens.get(*k).is_some_and(|t| t.is("!OR") || t.is("|")) {
            *k += 1;
            let right = self.and(tokens, k, env)?;
            left = flag(truth(&left) || truth(&right));
        }
        Ok(left)
    }

    fn and(&self, tokens: &[Tok], k: &mut usize, env: &mut Env) -> MResult<String> {
        let mut left = self.not(tokens, k, env)?;
        while tokens.get(*k).is_some_and(|t| t.is("!AND") || t.is("&")) {
            *k += 1;
            let right = self.not(tokens, k, env)?;
            left = flag(truth(&left) && truth(&right));
        }
        Ok(left)
    }

    fn not(&self, tokens: &[Tok], k: &mut usize, env: &mut Env) -> MResult<String> {
        if tokens
            .get(*k)
            .is_some_and(|t| t.is("!NOT") || t.is("~") || t.is("¬"))
        {
            *k += 1;
            let value = self.not(tokens, k, env)?;
            return Ok(flag(!truth(&value)));
        }
        let left = self.primary(tokens, k, env)?;
        let Some(op) = tokens.get(*k) else {
            return Ok(left);
        };
        let op = match op.upper().as_str() {
            "=" | "!EQ" => "=",
            "~=" | "¬=" | "<>" | "!NE" => "<>",
            "<" | "!LT" => "<",
            ">" | "!GT" => ">",
            "<=" | "!LE" => "<=",
            ">=" | "!GE" => ">=",
            _ => return Ok(left),
        };
        *k += 1;
        let right = self.primary(tokens, k, env)?;
        let (a, b) = (unquote(&left), unquote(&right));
        let order = match (a.trim().parse::<f64>(), b.trim().parse::<f64>()) {
            (Ok(x), Ok(y)) => x.partial_cmp(&y),
            _ => Some(a.cmp(&b)),
        };
        let holds = order.is_some_and(|o| match op {
            "=" => o.is_eq(),
            "<>" => o.is_ne(),
            "<" => o.is_lt(),
            ">" => o.is_gt(),
            "<=" => o.is_le(),
            _ => o.is_ge(),
        });
        Ok(flag(holds))
    }

    fn primary(&self, tokens: &[Tok], k: &mut usize, env: &mut Env) -> MResult<String> {
        if tokens.get(*k).is_some_and(|t| t.is("(")) {
            *k += 1;
            let value = self.expression(tokens, k, env)?;
            if tokens.get(*k).is_none_or(|t| !t.is(")")) {
                return fail(
                    tokens.get(*k).or(tokens.last()),
                    "expected ) in the macro expression",
                );
            }
            *k += 1;
            return Ok(value);
        }
        self.operand(tokens, k, env)
    }

    /// One operand: a token, an argument or variable, or a function call. Its value as text,
    /// quotes kept.
    fn operand(&self, tokens: &[Tok], k: &mut usize, env: &mut Env) -> MResult<String> {
        let Some(t) = tokens.get(*k) else {
            return fail(tokens.last(), "expected a value in the macro expression");
        };
        *k += 1;
        if t.kind != Kind::Bang {
            return Ok(t.text.clone());
        }
        if let Some(value) = env.args.get(&t.text).or_else(|| env.vars.get(&t.text)) {
            return Ok(value.clone());
        }
        let name = t.text.as_str();
        if name == "!NULL" {
            return Ok(String::new());
        }
        const FUNCTIONS: [&str; 11] = [
            "!LENGTH", "!CONCAT", "!SUBSTR", "!INDEX", "!HEAD", "!TAIL", "!QUOTE", "!UNQUOTE",
            "!UPCASE", "!BLANKS", "!EVAL",
        ];
        if !FUNCTIONS.contains(&name) {
            // a macro name or other text: expanded, if at all, when the result is scanned again
            return Ok(t.text.clone());
        }
        if tokens.get(*k).is_none_or(|p| !p.is("(")) {
            return fail(Some(t), format!("{name} needs ( after it"));
        }
        *k += 1;
        let mut args: Vec<String> = Vec::new();
        let mut current: Vec<String> = Vec::new();
        loop {
            let Some(next) = tokens.get(*k) else {
                return fail(Some(t), format!("{name} is not closed with )"));
            };
            if next.is(")") || next.is(",") {
                *k += 1;
                args.push(current.join(" "));
                current.clear();
                if next.is(")") {
                    break;
                }
                continue;
            }
            if next.kind == Kind::Newline {
                *k += 1;
                continue;
            }
            current.push(self.operand(tokens, k, env)?);
        }
        let count = |range: std::ops::RangeInclusive<usize>| -> MResult<()> {
            if range.contains(&args.len()) {
                return Ok(());
            }
            let (low, high) = (range.start(), range.end());
            let takes = if low == high {
                low.to_string()
            } else {
                format!("{low} to {high}")
            };
            fail(
                Some(t),
                format!("{name} takes {takes} arguments, got {}", args.len()),
            )
        };
        let number = |text: &str| -> MResult<usize> {
            unquote(text)
                .trim()
                .parse::<usize>()
                .or_else(|_| fail(Some(t), format!("{name} needs a whole number, got {text}")))
        };
        Ok(match name {
            "!LENGTH" => {
                count(1..=1)?;
                args[0].chars().count().to_string()
            }
            "!CONCAT" => args.iter().map(|a| unquote(a)).collect(),
            "!SUBSTR" => {
                count(2..=3)?;
                let chars: Vec<char> = args[0].chars().collect();
                let from = number(&args[1])?.max(1) - 1;
                let length = match args.get(2) {
                    Some(n) => number(n)?,
                    None => chars.len(),
                };
                chars.iter().skip(from).take(length).collect()
            }
            "!INDEX" => {
                count(2..=2)?;
                match args[0].find(args[1].as_str()) {
                    Some(byte) if !args[1].is_empty() => {
                        (args[0][..byte].chars().count() + 1).to_string()
                    }
                    _ => "0".to_string(),
                }
            }
            "!HEAD" | "!TAIL" => {
                count(1..=1)?;
                let tokens = tokenize(&unquote(&args[0]), false);
                match (name, tokens.split_first()) {
                    ("!HEAD", Some((head, _))) => head.text.clone(),
                    (_, Some((_, tail))) => render(tail).trim().to_string(),
                    _ => String::new(),
                }
            }
            "!QUOTE" => {
                count(1..=1)?;
                quote(&args[0])
            }
            "!UNQUOTE" => {
                count(1..=1)?;
                unquote(&args[0])
            }
            "!UPCASE" => {
                count(1..=1)?;
                unquote(&args[0]).to_uppercase()
            }
            "!BLANKS" => {
                count(1..=1)?;
                " ".repeat(number(&args[0])?)
            }
            _ => {
                count(1..=1)?;
                let tokens = tokenize(&args[0], true);
                self.expand(&tokens, 1)?.unwrap_or_else(|| args[0].clone())
            }
        })
    }
}

fn truth(value: &str) -> bool {
    unquote(value).trim() != "0"
}

fn flag(value: bool) -> String {
    if value { "1" } else { "0" }.to_string()
}

/// The index of the token that closes the block opened just before `from` (`close`, at the
/// same nesting of `open`), and of a `middle` token at that nesting before it.
fn matching(
    tokens: &[Tok],
    from: usize,
    open: &str,
    close: &str,
    middle: Option<&str>,
) -> Option<(Option<usize>, usize)> {
    let mut depth = 0;
    let mut found = None;
    for (j, t) in tokens.iter().enumerate().skip(from) {
        if t.is(open) {
            depth += 1;
        } else if t.is(close) {
            if depth == 0 {
                return Some((found, j));
            }
            depth -= 1;
        } else if depth == 0 && found.is_none() && middle.is_some_and(|m| t.is(m)) {
            found = Some(j);
        }
    }
    None
}

/// The expansion without blank lines, which would end the command, and without line breaks
/// at either end.
fn collapse(text: &str) -> String {
    let lines: Vec<&str> = text
        .split('\n')
        .map(str::trim_end)
        .filter(|l| !l.trim().is_empty())
        .collect();
    lines.join("\n").trim().to_string()
}
