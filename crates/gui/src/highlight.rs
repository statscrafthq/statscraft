//! Syntax colours: the spans of every line by kind, carried from line to line, and the layout
//! job the editor draws, kept for as long as the text and the theme stay the same.

use eframe::egui::{
    self, Color32, FontId, TextFormat,
    text::{ByteIndex, LayoutJob, LayoutSection},
};
use std::collections::HashMap;
use std::ops::Range;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Command,
    Subcommand,
    Keyword,
    String,
    Number,
    Comment,
    Macro,
    /// The period that ends a command.
    Period,
    Plain,
}

/// Where a line starts: inside a command or not, and which command.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct State {
    pub in_command: bool,
    /// The command's name, when its first words name one.
    pub command: Option<&'static str>,
    comment: bool,
    /// The lines between BEGIN DATA and END DATA.
    data: bool,
}

/// Words of every command that are no keywords of a single one.
const GENERAL: &[&str] = &[
    "ALL", "AND", "BY", "ELSE", "EQ", "GE", "GT", "HI", "HIGHEST", "INTO", "LE", "LO", "LOWEST",
    "LT", "MISSING", "NE", "NOT", "OR", "SYSMIS", "THRU", "TO", "WITH", "COPY",
];

/// Subcommands and keywords.
type Words = (Vec<String>, Vec<String>);

/// The subcommands and keywords of a command, read from its syntax diagram in the command
/// table: upper-case words, a word after `/` being a subcommand.
pub fn words_of(command: &str) -> &'static Words {
    static TABLE: OnceLock<HashMap<&'static str, Words>> = OnceLock::new();
    static NONE: Words = (Vec::new(), Vec::new());
    let table = TABLE.get_or_init(|| {
        statscraft_syntax::command_specs()
            .iter()
            .map(|spec| (spec.name, diagram_words(spec.name, spec.syntax)))
            .collect()
    });
    table.get(command).unwrap_or(&NONE)
}

fn diagram_words(name: &str, syntax: &str) -> Words {
    let (mut subcommands, mut keywords): Words = Default::default();
    let bytes = syntax.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if !bytes[i].is_ascii_alphabetic() {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
            i += 1;
        }
        let word = &syntax[start..i];
        let upper = word.len() >= 2 && word.bytes().all(|b| b.is_ascii_uppercase() || b == b'_');
        if !upper || name.split([' ', '-']).any(|w| w == word) {
            continue;
        }
        let before = syntax[..start].trim_end_matches([' ', ']', '[']);
        let list = if before.ends_with('/') {
            &mut subcommands
        } else {
            &mut keywords
        };
        if !list.iter().any(|w| w == word) {
            list.push(word.to_string());
        }
    }
    subcommands.sort();
    keywords.retain(|k| !subcommands.contains(k));
    keywords.sort();
    (subcommands, keywords)
}

fn is_keyword(command: Option<&str>, word: &str) -> bool {
    let upper = word.to_ascii_uppercase();
    GENERAL.contains(&upper.as_str())
        || command.is_some_and(|c| {
            let (subcommands, keywords) = words_of(c);
            keywords.contains(&upper) || subcommands.contains(&upper)
        })
}

/// The period at the end of a line that ends a command, as a byte index: the last character
/// that is not blank, outside quotes, after `/* */` comments are taken out.
fn terminator(line: &str) -> Option<usize> {
    let mut last = None;
    let mut quote = None;
    let mut chars = line.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
        } else if c == '/' && chars.peek().is_some_and(|&(_, n)| n == '*') {
            chars.next();
            let rest = &line[i + 2..];
            match rest.find("*/") {
                Some(end) => {
                    while chars.peek().is_some_and(|&(j, _)| j < i + 2 + end + 2) {
                        chars.next();
                    }
                }
                None => break,
            }
            continue;
        } else if c == '\'' || c == '"' {
            quote = Some(c);
        }
        if !c.is_whitespace() {
            last = Some((i, c));
        }
    }
    last.filter(|&(_, c)| c == '.').map(|(i, _)| i)
}

fn word_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '.' | '$' | '#' | '@' | '!')
}

/// The end of the word at `start`: word characters, without a period that ends it.
fn word_end(line: &str, start: usize, stop: usize) -> usize {
    let end = line[start..stop]
        .find(|c: char| !word_char(c))
        .map_or(stop, |n| start + n);
    start + line[start..end].trim_end_matches('.').len().max(1)
}

/// The words of a command name at `start` of the line, as byte ranges: words separated by
/// blanks, `T-TEST` being one.
fn name_words(line: &str, start: usize, stop: usize) -> Vec<Range<usize>> {
    let mut words = Vec::new();
    let mut at = start;
    while words.len() < 4 && at < stop && line[at..].starts_with(char::is_alphabetic) {
        let end = line[at..stop]
            .find(|c: char| !(c.is_alphanumeric() || c == '-' || c == '_'))
            .map_or(stop, |n| at + n);
        words.push(at..end);
        let blank = line[end..stop].len() - line[end..stop].trim_start().len();
        if blank == 0 {
            break;
        }
        at = end + blank;
    }
    words
}

/// The spans of one line that are not plain, in order, and the state of the next line.
pub fn line(text: &str, state: &mut State) -> Vec<(Range<usize>, Kind)> {
    let mut spans = Vec::new();
    let code = text.trim();
    if state.data {
        if is_end_data(code) {
            spans.push((0..text.len(), Kind::Command));
            *state = State::default();
        }
        return spans;
    }
    if code.is_empty() {
        *state = State::default();
        return spans;
    }
    let indent = text.len() - text.trim_start().len();
    if !state.in_command {
        let first = name_words(text, indent, text.len());
        let comment_word = first.first().is_some_and(|w| {
            let w = &text[w.clone()];
            w.len() >= 4 && "COMMENT".starts_with(&w.to_ascii_uppercase())
        });
        if code.starts_with('*') || comment_word {
            state.comment = true;
        }
    }
    if state.comment {
        spans.push((indent..text.len(), Kind::Comment));
        state.in_command = !code.ends_with('.');
        state.comment = state.in_command;
        return spans;
    }
    let period = terminator(text);
    let stop = period.unwrap_or(text.len());
    let mut i = indent;
    if !state.in_command {
        state.in_command = true;
        state.command = None;
        let words = name_words(text, indent, stop);
        let strs: Vec<&str> = words.iter().map(|w| &text[w.clone()]).collect();
        if let Some((name, used)) = statscraft_syntax::command_name(&strs) {
            state.command = Some(name);
            spans.push((words[0].start..words[used - 1].end, Kind::Command));
            i = words[used - 1].end;
            state.data = name == "BEGIN DATA";
        }
    }
    let mut after_slash = false;
    while i < stop {
        let rest = &text[i..stop];
        let c = rest.chars().next().expect("not at the end");
        if c.is_whitespace() {
            i += c.len_utf8();
            continue;
        }
        if rest.starts_with("/*") {
            let end = rest.find("*/").map_or(stop, |n| i + n + 2);
            spans.push((i..end, Kind::Comment));
            i = end;
            continue;
        }
        if c == '\'' || c == '"' {
            let mut end = stop;
            let mut chars = rest.char_indices().skip(1).peekable();
            while let Some((n, d)) = chars.next() {
                if d == c {
                    // a doubled quote stays inside the string
                    if chars.peek().is_some_and(|&(_, e)| e == c) {
                        chars.next();
                        continue;
                    }
                    end = i + n + 1;
                    break;
                }
            }
            spans.push((i..end, Kind::String));
            i = end;
            after_slash = false;
            continue;
        }
        let number_start =
            c.is_ascii_digit() || (c == '.' && rest[1..].starts_with(|d: char| d.is_ascii_digit()));
        if number_start {
            let mut end = i;
            let mut exponent = false;
            for (n, d) in rest.char_indices() {
                let sign = matches!(d, '+' | '-') && exponent;
                if d.is_ascii_digit() || d == '.' || sign {
                    exponent = false;
                } else if matches!(d, 'e' | 'E') {
                    exponent = true;
                } else {
                    break;
                }
                end = i + n + 1;
            }
            spans.push((i..end, Kind::Number));
            i = end;
            after_slash = false;
            continue;
        }
        if word_char(c) && c != '.' {
            let end = word_end(text, i, stop);
            let word = &text[i..end];
            let kind = if c == '!' {
                Kind::Macro
            } else if after_slash {
                Kind::Subcommand
            } else if is_keyword(state.command, word) {
                Kind::Keyword
            } else {
                Kind::Plain
            };
            if kind != Kind::Plain {
                spans.push((i..end, kind));
            }
            i = end;
            after_slash = false;
            continue;
        }
        after_slash = c == '/';
        i += c.len_utf8();
    }
    if let Some(p) = period {
        spans.push((p..p + 1, Kind::Period));
        let data = state.data;
        *state = State {
            data,
            ..State::default()
        };
    }
    spans
}

fn is_end_data(code: &str) -> bool {
    let code = code.strip_suffix('.').unwrap_or(code);
    let mut words = code.split_whitespace();
    words.next().is_some_and(|w| w.eq_ignore_ascii_case("END"))
        && words.next().is_some_and(|w| w.eq_ignore_ascii_case("DATA"))
        && words.next().is_none()
}

/// The state at the start of the line that holds byte `at`, and that line's start.
pub fn state_at(text: &str, at: usize) -> (State, usize) {
    let mut state = State::default();
    let mut start = 0;
    for l in text.split_inclusive('\n') {
        if start + l.len() > at || !l.ends_with('\n') {
            break;
        }
        line(l.trim_end_matches(['\n', '\r']), &mut state);
        start += l.len();
    }
    (state, start)
}

/// Colours of a theme, light or dark.
pub fn color(kind: Kind, dark: bool, plain: Color32) -> Color32 {
    let rgb = |r, g, b| Color32::from_rgb(r, g, b);
    match (kind, dark) {
        (Kind::Plain, _) => plain,
        (Kind::Command, false) => rgb(0x1a, 0x4f, 0xb8),
        (Kind::Command, true) => rgb(0x6c, 0xb0, 0xff),
        (Kind::Subcommand, false) => rgb(0x00, 0x74, 0x74),
        (Kind::Subcommand, true) => rgb(0x4e, 0xd0, 0xc0),
        (Kind::Keyword, false) => rgb(0x86, 0x2d, 0xa8),
        (Kind::Keyword, true) => rgb(0xd0, 0x96, 0xe8),
        (Kind::String, false) => rgb(0xa3, 0x3a, 0x12),
        (Kind::String, true) => rgb(0xe8, 0xa0, 0x78),
        (Kind::Number, false) => rgb(0x0b, 0x7a, 0x3e),
        (Kind::Number, true) => rgb(0xa8, 0xd8, 0x8c),
        (Kind::Comment, false) => rgb(0x6a, 0x70, 0x78),
        (Kind::Comment, true) => rgb(0x8c, 0x94, 0x9c),
        (Kind::Macro, false) => rgb(0x8a, 0x5a, 0x00),
        (Kind::Macro, true) => rgb(0xe6, 0xc8, 0x70),
        (Kind::Period, false) => rgb(0xc0, 0x10, 0x10),
        (Kind::Period, true) => rgb(0xff, 0x80, 0x70),
    }
}

/// The whole text as a layout job with its colours; the wrap width is set by the caller.
fn job(text: &str, dark: bool, plain: Color32, size: f32) -> LayoutJob {
    let font = FontId::monospace(size);
    let format = |kind: Kind| TextFormat {
        font_id: font.clone(),
        color: color(kind, dark, plain),
        italics: kind == Kind::Comment,
        ..Default::default()
    };
    let mut job = LayoutJob {
        text: text.to_string(),
        ..Default::default()
    };
    let mut push = |range: Range<usize>, kind: Kind| {
        if range.is_empty() {
            return;
        }
        if let Some(last) = job.sections.last_mut()
            && last.byte_range.end.0 == range.start
            && last.format == format(kind)
        {
            last.byte_range.end = ByteIndex(range.end);
            return;
        }
        job.sections.push(LayoutSection {
            leading_space: 0.0,
            byte_range: ByteIndex(range.start)..ByteIndex(range.end),
            format: format(kind),
        });
    };
    let mut state = State::default();
    let mut start = 0;
    for l in text.split_inclusive('\n') {
        let mut at = start;
        for (range, kind) in line(l.trim_end_matches(['\n', '\r']), &mut state) {
            push(at..start + range.start, Kind::Plain);
            push(start + range.start..start + range.end, kind);
            at = start + range.end;
        }
        push(at..start + l.len(), Kind::Plain);
        start += l.len();
    }
    job
}

#[derive(Default)]
struct Highlighter;

impl egui::cache::ComputerMut<(&str, bool, Color32, u32), LayoutJob> for Highlighter {
    fn compute(&mut self, (text, dark, plain, size): (&str, bool, Color32, u32)) -> LayoutJob {
        job(text, dark, plain, size as f32)
    }
}

/// The coloured layout job of `text`, computed again only when the text or the theme changes.
pub fn cached_job(
    ctx: &egui::Context,
    text: &str,
    dark: bool,
    plain: Color32,
    size: f32,
) -> LayoutJob {
    type Cache = egui::cache::FrameCache<LayoutJob, Highlighter>;
    ctx.memory_mut(|m| {
        m.caches
            .cache::<Cache>()
            .get((text, dark, plain, size as u32))
            .clone()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(text: &str) -> Vec<(&str, Kind)> {
        let mut state = State::default();
        let mut out = Vec::new();
        for l in text.lines() {
            for (range, kind) in line(l, &mut state) {
                out.push((&l[range], kind));
            }
        }
        out
    }

    #[test]
    fn a_line_has_its_command_subcommands_keywords_and_period() {
        assert_eq!(
            kinds("FREQUENCIES /VARIABLES=age TO q5."),
            [
                ("FREQUENCIES", Kind::Command),
                ("VARIABLES", Kind::Subcommand),
                ("TO", Kind::Keyword),
                (".", Kind::Period),
            ]
        );
        assert_eq!(
            kinds("var lab x 'It''s \"x\"' /* note */ y \"two\"."),
            [
                ("var lab", Kind::Command),
                ("'It''s \"x\"'", Kind::String),
                ("/* note */", Kind::Comment),
                ("\"two\"", Kind::String),
                (".", Kind::Period),
            ]
        );
        assert_eq!(
            kinds("compute y = 1.5e3 + .5 * !n."),
            [
                ("compute", Kind::Command),
                ("1.5e3", Kind::Number),
                (".5", Kind::Number),
                ("!n", Kind::Macro),
                (".", Kind::Period),
            ]
        );
        // T-TEST is one name; a period inside a name such as CHAR.LENGTH is no terminator
        assert_eq!(
            kinds("T-TEST GROUPS=g(1 2) /VARIABLES=x.\nCOMPUTE n = CHAR.LENGTH(s)."),
            [
                ("T-TEST", Kind::Command),
                ("GROUPS", Kind::Keyword),
                ("1", Kind::Number),
                ("2", Kind::Number),
                ("VARIABLES", Kind::Subcommand),
                (".", Kind::Period),
                ("COMPUTE", Kind::Command),
                (".", Kind::Period),
            ]
        );
    }

    #[test]
    fn state_carries_over_lines() {
        // a command runs on until its period; the next line starts a new one
        assert_eq!(
            kinds("FREQUENCIES\n  /VARIABLES=x.\nfre y"),
            [
                ("FREQUENCIES", Kind::Command),
                ("VARIABLES", Kind::Subcommand),
                (".", Kind::Period),
                ("fre", Kind::Command),
            ]
        );
        // a blank line ends a command; comments run to their period
        assert_eq!(
            kinds("fre x\n\n* a note\n  on two lines.\nCOMMENT more.\nexecute."),
            [
                ("fre", Kind::Command),
                ("* a note", Kind::Comment),
                ("on two lines.", Kind::Comment),
                ("COMMENT more.", Kind::Comment),
                ("execute", Kind::Command),
                (".", Kind::Period),
            ]
        );
        // inline data is left plain
        assert_eq!(
            kinds("BEGIN DATA\n1 'a'.\nEND DATA.\nEXECUTE."),
            [
                ("BEGIN DATA", Kind::Command),
                ("END DATA.", Kind::Command),
                ("EXECUTE", Kind::Command),
                (".", Kind::Period),
            ]
        );
        let text = "GET FILE='a.sav'.\nFREQUENCIES\n  VARIABLES=x";
        let (state, start) = state_at(text, text.len());
        assert_eq!(start, text.rfind('\n').unwrap() + 1);
        assert_eq!(state.command, Some("FREQUENCIES"));
        assert!(state.in_command);
    }

    #[test]
    fn keywords_come_from_the_syntax_diagram() {
        let (subcommands, keywords) = words_of("SAVE");
        assert_eq!(subcommands, &["COMPRESSED", "UNCOMPRESSED", "ZCOMPRESSED"]);
        assert_eq!(keywords, &["OUTFILE"]);
        assert!(words_of("FREQUENCIES").0.contains(&"VARIABLES".to_string()));
        assert_eq!(words_of("NO SUCH"), &(Vec::new(), Vec::new()));
    }

    #[test]
    fn the_job_covers_the_text_once() {
        let text = "fre x.\n* note.\n\nCOMPUTE y = 'a'.\n";
        let job = job(text, true, Color32::WHITE, 14.0);
        let mut at = 0;
        for section in &job.sections {
            assert_eq!(section.byte_range.start.0, at);
            at = section.byte_range.end.0;
        }
        assert_eq!(at, text.len());
    }
}
