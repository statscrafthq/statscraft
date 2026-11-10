//! What happens to the source before it is split into commands: macro definitions are taken
//! out and macro calls expanded, INSERT and INCLUDE bring in the text of other syntax files,
//! the bodies of BEGIN PROGRAM blocks are set aside, and BEGIN DATA blocks pass unchanged.
//!
//! Every line that comes out carries the number of the source line it stands for: the line of
//! the call for an expansion, the line of the INSERT for an inserted file. Its `origin` says
//! where the text really comes from, for error messages.
//!
//! Inserted files: a relative path is relative to the folder of the file that holds the INSERT
//! (the current folder for syntax that is not in a file), or to the folder a CD command named
//! before. INCLUDE and INSERT SYNTAX=BATCH read the file by the batch rules: a line that starts
//! in the first column starts a new command, and `+`, `-` or `.` there only indents it. Files
//! nest up to five levels and a file may not insert itself.

use crate::error::{Span, SyntaxError};
use crate::macros::{Kind, Macros, Tok, define, tokenize};
use crate::split::{Start, command_end, is_line_command};
use crate::table::{Resolution, resolve};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// One line of the text that is split into commands.
#[derive(Debug, Clone)]
pub(crate) struct Line {
    pub number: u32,
    pub text: String,
    /// Where the text comes from when it is not the line itself, such as
    /// "the expansion of macro !TABLES" or "lib.sps, line 12".
    pub origin: Option<String>,
    /// With SET MPRINT=YES, the command as the macros expanded it.
    pub echo: Option<String>,
}

pub(crate) enum Piece {
    Line(Line),
    Error(SyntaxError),
}

const MAX_NESTING: usize = 5;

/// The macros defined and the SET values that steer their expansion (MEXPAND, MPRINT, MNEST,
/// MITERATE). A session keeps it between runs, so that a macro library included once serves
/// the runs after it.
#[derive(Clone, Debug)]
pub struct MacroState {
    pub(crate) macros: Macros,
    /// The text of every definition, by upper-case name, to write the state out.
    sources: BTreeMap<String, String>,
    /// SET MEXPAND
    pub(crate) expand: bool,
    /// SET MPRINT
    pub(crate) print: bool,
}

impl Default for MacroState {
    fn default() -> Self {
        Self {
            macros: Macros::default(),
            sources: BTreeMap::new(),
            expand: true,
            print: false,
        }
    }
}

impl PartialEq for MacroState {
    fn eq(&self, other: &Self) -> bool {
        self.to_syntax() == other.to_syntax()
    }
}

impl MacroState {
    /// The names of the defined macros, sorted.
    pub fn names(&self) -> Vec<String> {
        self.sources.keys().cloned().collect()
    }

    /// The state as syntax that `from_syntax` reads back: every definition, then one SET.
    pub fn to_syntax(&self) -> String {
        let mut out = String::new();
        for text in self.sources.values() {
            out.push_str(text);
            out.push('\n');
        }
        let on = |b: bool| if b { "ON" } else { "OFF" };
        out.push_str(&format!(
            "SET MEXPAND={} MPRINT={} MNEST={} MITERATE={}.\n",
            on(self.expand),
            on(self.print),
            self.macros.nest,
            self.macros.iterate
        ));
        out
    }

    /// The state that `to_syntax` wrote.
    pub fn from_syntax(text: &str) -> Self {
        let mut state = Self::default();
        preprocess_with(text, None, &mut state, None);
        state
    }

    fn define(&mut self, definition: crate::macros::Macro, text: String) {
        self.sources.insert(definition.name.clone(), text);
        self.macros
            .defined
            .insert(definition.name.clone(), definition);
    }
}

struct Pre {
    /// What this text sees: the state it started from and every definition and SET in it.
    state: MacroState,
    /// What a session keeps: the state it started from and the definitions and SETs of the
    /// lines that run.
    kept: MacroState,
    /// The source lines that run; None for all.
    range: Option<(u32, u32)>,
    out: Vec<Piece>,
    /// The inserted files being read, outermost first.
    open: Vec<PathBuf>,
}

/// Where the lines of a text are in the source.
#[derive(Clone)]
enum Place {
    /// The source itself: line n is line n.
    Source,
    /// An inserted file: every line stands for the INSERT on this source line.
    Inserted { line: u32, file: String },
}

/// The source with macros expanded and files inserted, as numbered lines and the errors found
/// on the way, in source order. `base` is the folder of the syntax file, if it is one.
#[cfg(test)]
pub(crate) fn preprocess(source: &str, base: Option<&Path>) -> Vec<Piece> {
    preprocess_with(source, base, &mut MacroState::default(), None)
}

/// `preprocess` starting from the macros of `state`. Afterwards `state` holds them and those
/// defined (and the macro settings changed) on the source lines of `range`, or anywhere when
/// it is None. Definitions outside the range still serve calls inside it, as before.
pub(crate) fn preprocess_with(
    source: &str,
    base: Option<&Path>,
    state: &mut MacroState,
    range: Option<(u32, u32)>,
) -> Vec<Piece> {
    let mut pre = Pre {
        state: state.clone(),
        kept: state.clone(),
        range,
        out: Vec::new(),
        open: Vec::new(),
    };
    let mut base = base.map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    pre.text(source, &mut base, &Place::Source, false);
    *state = pre.kept;
    pre.out
}

/// The leading words of a line in upper case, for recognising commands.
fn leading_words(line: &str) -> Vec<String> {
    tokenize(line, false)
        .into_iter()
        .take_while(|t| t.kind == Kind::Id)
        .take(3)
        .map(|t| t.text.to_uppercase())
        .collect()
}

fn command_name(words: &[String]) -> Option<&'static str> {
    let words: Vec<&str> = words.iter().map(String::as_str).collect();
    match resolve(&words) {
        Resolution::Supported(spec, _) => Some(spec.name),
        Resolution::Unsupported(name, _) => Some(name),
        _ => None,
    }
}

fn starts_with(words: &[String], expected: &[&str]) -> bool {
    words.len() >= expected.len() && words.iter().zip(expected).all(|(w, e)| w == e)
}

/// The text read by the batch rules, rewritten for the interactive ones: a blank line before
/// every command, and the indent marker in the first column taken away.
fn batch_to_interactive(
    lines: Vec<(u32, String, Option<String>)>,
) -> Vec<(u32, String, Option<String>)> {
    let mut out: Vec<(u32, String, Option<String>)> = Vec::new();
    for (number, mut text, origin) in lines {
        let first = text.chars().next();
        if first.is_some_and(|c| !c.is_whitespace()) {
            if matches!(first, Some('+' | '-' | '.')) {
                text.replace_range(..1, " ");
            }
            if out.last().is_some_and(|l| !l.1.trim().is_empty()) {
                out.push((number, String::new(), None));
            }
        }
        out.push((number, text, origin));
    }
    out
}

fn resolve_path(base: &Path, file: &str) -> PathBuf {
    let path = Path::new(file);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}

impl Pre {
    /// Whether source line `number` runs, so that what it defines is kept.
    fn runs(&self, number: u32) -> bool {
        self.range
            .is_none_or(|(first, last)| (first..=last).contains(&number))
    }

    fn emit(&mut self, number: u32, text: &str, origin: &Option<String>) {
        self.out.push(Piece::Line(Line {
            number,
            text: text.to_string(),
            origin: origin.clone(),
            echo: None,
        }));
    }

    fn error(&mut self, line: u32, column: u32, message: String, origin: &Option<String>) {
        let span = Span {
            line,
            column,
            end_line: line,
            end_column: column + 1,
        };
        let mut error = SyntaxError::new(span, message);
        if let Some(origin) = origin {
            error = error.hint(format!("in {origin}"));
        }
        self.out.push(Piece::Error(error));
    }

    fn text(&mut self, text: &str, base: &mut PathBuf, place: &Place, batch: bool) {
        let mut lines: Vec<(u32, String, Option<String>)> = text
            .lines()
            .zip(1u32..)
            .map(|(line, n)| match place {
                Place::Source => (n, line.to_string(), None),
                Place::Inserted { line: at, file } => {
                    (*at, line.to_string(), Some(format!("{file}, line {n}")))
                }
            })
            .collect();
        if batch {
            lines = batch_to_interactive(lines);
        }
        let plain: Vec<(u32, &str)> = lines.iter().map(|l| (l.0, l.1.as_str())).collect();
        let origin = |i: usize| lines[i].2.clone();
        let mut i = 0;
        while i < plain.len() {
            let (number, line) = plain[i];
            if line.trim().is_empty() {
                self.emit(number, line, &origin(i));
                i += 1;
                continue;
            }
            let words = leading_words(line);
            if words.first().is_some_and(|w| w == "DEFINE") {
                i = self.define(&plain, i, &origin(i));
                continue;
            }
            let data = is_line_command(line, ["BEGIN", "DATA"]);
            if data || starts_with(&words, &["BEGIN", "PROGRAM"]) {
                let close = if data { "END DATA" } else { "END PROGRAM" };
                let end = (i + 1..plain.len()).find(|&j| {
                    let words = leading_words(plain[j].1);
                    match data {
                        true => is_line_command(plain[j].1, ["END", "DATA"]),
                        false => starts_with(&words, &["END", "PROGRAM"]),
                    }
                });
                if data {
                    // the data lines pass as they are; the splitter reports a missing END DATA
                    let last = end.unwrap_or(plain.len() - 1);
                    for (j, &(n, text)) in plain.iter().enumerate().take(last + 1).skip(i) {
                        self.emit(n, text, &origin(j));
                    }
                    i = last + 1;
                    continue;
                }
                let Some(end) = end else {
                    self.error(
                        number,
                        1,
                        format!("BEGIN PROGRAM without {close}"),
                        &origin(i),
                    );
                    return;
                };
                {
                    // programs in other languages are not run: one command stands for the block
                    self.emit(number, "BEGIN PROGRAM", &origin(i));
                    self.emit(plain[end].0, ".", &origin(end));
                }
                i = end + 1;
                continue;
            }
            let (end, terminator, kind) = command_end(&plain, i);
            let name = command_name(&words);
            if kind == Start::Comment {
                for (j, &(n, text)) in plain.iter().enumerate().take(end).skip(i) {
                    self.emit(n, text, &origin(j));
                }
                i = end;
                continue;
            }
            let mut text: Vec<&str> = plain[i..end].iter().map(|l| l.1).collect();
            if let (Some((byte, _)), Some(last)) = (terminator, text.last_mut()) {
                *last = &last[..byte];
            }
            let tokens = tokenize(&text.join("\n"), true);
            match name {
                Some("INSERT" | "INCLUDE") => {
                    for (j, &(n, text)) in plain.iter().enumerate().take(end).skip(i) {
                        self.emit(n, text, &origin(j));
                    }
                    self.insert(&tokens, name == Some("INCLUDE"), base, number, &origin(i));
                    i = end;
                    continue;
                }
                Some("CD") => {
                    if let Some(dir) = tokens.iter().find(|t| t.kind == Kind::Str) {
                        *base = resolve_path(base, &crate::macros::unquote(&dir.text));
                    }
                }
                Some("SET") => {
                    let keep = self.runs(number);
                    set(&mut self.state, &tokens);
                    if keep {
                        set(&mut self.kept, &tokens);
                    }
                }
                _ => {}
            }
            let expanded = if self.state.expand {
                self.state.macros.expand(&tokens, 0)
            } else {
                Ok(None)
            };
            match expanded {
                Ok(None) => {
                    for (j, &(n, text)) in plain.iter().enumerate().take(end).skip(i) {
                        self.emit(n, text, &origin(j));
                    }
                }
                Ok(Some(text)) => {
                    self.emit_expanded(&plain[i..end], &text, terminator, &tokens, &origin(i))
                }
                Err((at, message)) => {
                    let (line, column) =
                        at.map_or((number, 1), |(l, c)| (plain[i + l as usize].0, c));
                    self.error(line, column, message, &origin(i));
                }
            }
            i = end;
        }
    }

    /// The lines of a command whose macro calls were expanded. Expansion lines (marked `\r`)
    /// carry the number of the line they replace.
    fn emit_expanded(
        &mut self,
        lines: &[(u32, &str)],
        text: &str,
        terminator: Option<(usize, u32)>,
        tokens: &[Tok],
        origin: &Option<String>,
    ) {
        let called: Vec<String> = tokens
            .iter()
            .filter(|t| self.state.macros.is_macro(t))
            .map(|t| t.text.to_uppercase())
            .collect();
        let what = format!("the expansion of macro {}", called.join(", "));
        let origin = Some(match origin {
            Some(file) => format!("{file}, {what}"),
            None => what,
        });
        let first = self.out.len();
        for (k, line) in text.split('\n').enumerate() {
            let number = lines[k.min(lines.len() - 1)].0;
            for part in line.split('\r') {
                self.emit(number, part, &origin);
            }
        }
        if terminator.is_some() {
            let last = self.out.len() - 1;
            if let Some(Piece::Line(line)) = self.out.get_mut(last) {
                if line.text.trim_end().ends_with('.') {
                    let number = line.number;
                    self.emit(number, ".", &origin);
                } else {
                    line.text.push('.');
                }
            }
        }
        if self.state.print
            && let Some(Piece::Line(line)) = self.out.get_mut(first)
        {
            line.echo = Some(text.replace('\r', "\n"));
        }
    }

    /// `DEFINE` at line `i`: registers the macro, puts one command in its place and returns
    /// the line after `!ENDDEFINE`.
    fn define(&mut self, lines: &[(u32, &str)], i: usize, origin: &Option<String>) -> usize {
        let number = lines[i].0;
        for j in i..lines.len() {
            if !lines[j].1.to_lowercase().contains("!enddefine") {
                continue;
            }
            let text: Vec<&str> = lines[i..=j].iter().map(|l| l.1).collect();
            let tokens = tokenize(&text.join("\n"), true);
            let Some(end) = tokens.iter().position(|t| t.is("!ENDDEFINE")) else {
                continue;
            };
            match define(&tokens[..=end]) {
                Ok(definition) => {
                    let placeholder = format!("DEFINE '{}'", definition.name);
                    let source = format!("{}.", text.join("\n").trim_end().trim_end_matches('.'));
                    if self.runs(number) {
                        self.kept.define(definition.clone(), source.clone());
                    }
                    self.state.define(definition, source);
                    if j == i {
                        self.emit(number, &format!("{placeholder}."), origin);
                    } else {
                        self.emit(number, &placeholder, origin);
                        self.emit(lines[j].0, ".", origin);
                    }
                }
                Err((at, message)) => {
                    let (line, column) =
                        at.map_or((number, 1), |(l, c)| (lines[i + l as usize].0, c));
                    self.error(line, column, message, origin);
                }
            }
            return j + 1;
        }
        self.error(number, 1, "DEFINE without !ENDDEFINE".into(), origin);
        lines.len()
    }

    /// The text of the file an INSERT or INCLUDE names, read in place of the command. A file
    /// that cannot be read is left to the command, which fails when it runs.
    fn insert(
        &mut self,
        tokens: &[Tok],
        include: bool,
        base: &mut PathBuf,
        line: u32,
        origin: &Option<String>,
    ) {
        let code: Vec<&Tok> = tokens.iter().filter(|t| t.kind != Kind::Newline).collect();
        let setting = |name: &str| {
            let k = code.iter().position(|t| t.kind == Kind::Id && t.is(name))?;
            let k = if code.get(k + 1).is_some_and(|t| t.is("=")) {
                k + 2
            } else {
                k + 1
            };
            code.get(k).map(|t| crate::macros::unquote(&t.text))
        };
        // FILE may be left out, as the command's parser allows: the path then follows the name
        let bare = code.get(1).filter(|t| t.kind == Kind::Str);
        let Some(file) = setting("FILE").or_else(|| bare.map(|t| crate::macros::unquote(&t.text)))
        else {
            return;
        };
        let batch = include || setting("SYNTAX").is_some_and(|s| s.eq_ignore_ascii_case("BATCH"));
        let path = resolve_path(base, &file);
        let Ok(bytes) = std::fs::read(&path) else {
            return;
        };
        let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
        if self.open.contains(&canonical) {
            self.error(line, 1, format!("{file} inserts itself"), origin);
            return;
        }
        if self.open.len() >= MAX_NESTING {
            self.error(
                line,
                1,
                format!("inserted files nest more than {MAX_NESTING} levels deep"),
                origin,
            );
            return;
        }
        let text = match crate::encoding::decode_syntax(&bytes, setting("ENCODING").as_deref()) {
            Ok(text) => text,
            Err(message) => {
                self.error(line, 1, message, origin);
                return;
            }
        };
        let folder = path
            .parent()
            .map_or_else(|| base.clone(), Path::to_path_buf);
        let mut inner = folder.clone();
        self.open.push(canonical);
        let place = Place::Inserted {
            line,
            file: match origin {
                Some(outer) => format!("{file} (from {outer})"),
                None => file.clone(),
            },
        };
        self.text(&text, &mut inner, &place, batch);
        self.open.pop();
        if setting("CD").is_some_and(|s| s.eq_ignore_ascii_case("YES")) {
            *base = folder;
        }
    }
}

/// SET MEXPAND, MPRINT, MNEST and MITERATE act while macros are expanded.
fn set(state: &mut MacroState, tokens: &[Tok]) {
    let code: Vec<&Tok> = tokens.iter().filter(|t| t.kind != Kind::Newline).collect();
    for (k, t) in code.iter().enumerate() {
        let value = |k: usize| {
            let k = if code.get(k).is_some_and(|t| t.is("=")) {
                k + 1
            } else {
                k
            };
            code.get(k).map(|t| t.text.to_uppercase())
        };
        let on = |v: Option<String>| v.is_some_and(|v| v == "ON" || v == "YES");
        // a limit that is not a positive whole number is left as it was; SET reports it
        let limit = |v: Option<String>, old: usize| {
            v.and_then(|v| v.parse::<usize>().ok())
                .filter(|&n| n > 0)
                .unwrap_or(old)
        };
        if t.is("MEXPAND") {
            state.expand = on(value(k + 1));
        } else if t.is("MPRINT") {
            state.print = on(value(k + 1));
        } else if t.is("MNEST") {
            state.macros.nest = limit(value(k + 1), state.macros.nest);
        } else if t.is("MITERATE") {
            state.macros.iterate = limit(value(k + 1), state.macros.iterate);
        }
    }
}
