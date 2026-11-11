//! Completion in the syntax editor: what fits at the cursor, from the command table, the open
//! dataset and the functions of expressions.

use crate::highlight::{self, words_of};
use std::ops::Range;

/// Commands whose body is an expression, where function names are offered.
const EXPRESSIONS: &[&str] = &["COMPUTE", "IF", "SELECT IF", "DO IF", "ELSE IF", "LOOP"];

/// The most candidates shown.
const MOST: usize = 200;

/// The words of the macro language, offered after `!`.
const MACRO_WORDS: &[&str] = &[
    "!BLANKS",
    "!BREAK",
    "!BY",
    "!CHAREND",
    "!CMDEND",
    "!CONCAT",
    "!DEFAULT",
    "!DO",
    "!DOEND",
    "!ELSE",
    "!ENCLOSE",
    "!ENDDEFINE",
    "!EVAL",
    "!HEAD",
    "!IF",
    "!IFEND",
    "!IN",
    "!INDEX",
    "!LENGTH",
    "!LET",
    "!NOEXPAND",
    "!NULL",
    "!OFFEXPAND",
    "!ONEXPAND",
    "!POSITIONAL",
    "!QUOTE",
    "!SUBSTR",
    "!TAIL",
    "!THEN",
    "!TO",
    "!TOKENS",
    "!UNQUOTE",
    "!UPCASE",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum What {
    Command,
    Subcommand,
    Keyword,
    Variable,
    Function,
    /// A macro defined with DEFINE, or a word of the macro language.
    Macro,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    /// What accepting it puts in the text.
    pub text: String,
    /// A label, a summary or the call form, shown beside it.
    pub detail: String,
    pub what: What,
}

/// The candidates for the cursor at byte `cursor`, and the bytes they replace. `explicit` (from
/// Ctrl+Space) offers candidates before anything is typed. `variables` holds name and label;
/// `macros` names the macros defined outside the text, in the session.
pub fn candidates(
    text: &str,
    cursor: usize,
    variables: &[(String, String)],
    macros: &[String],
    explicit: bool,
) -> (Vec<Candidate>, Range<usize>) {
    let before = &text[..cursor];
    let word_start = before
        .char_indices()
        .rev()
        .take_while(|&(_, c)| c.is_alphanumeric() || matches!(c, '_' | '.' | '$' | '#' | '@' | '!'))
        .last()
        .map_or(cursor, |(i, _)| i);
    let typed = &text[word_start..cursor];
    if typed.is_empty() && !explicit {
        return (Vec::new(), cursor..cursor);
    }
    let (state, line_start) = highlight::state_at(text, cursor);
    let line = &text[line_start..cursor];
    let mut macro_names = highlight::macro_names(text);
    for name in macros {
        if !macro_names.iter().any(|m| m.eq_ignore_ascii_case(name)) {
            macro_names.push(name.clone());
        }
    }
    let macro_calls = |typed: &str| -> Vec<Candidate> {
        macro_names
            .iter()
            .filter(|name| starts_with_ignore_case(name, typed))
            .map(|name| Candidate {
                text: name.clone(),
                detail: "macro".into(),
                what: What::Macro,
            })
            .collect()
    };

    // the first words of a command: its name, perhaps of several words, perhaps abbreviated
    let indent = line.len() - line.trim_start().len();
    let words: Vec<&str> = line.split_whitespace().collect();
    let only_words = line
        .trim()
        .chars()
        .all(|c| c.is_alphanumeric() || c.is_whitespace() || c == '-');
    if !state.in_command && only_words {
        let name_start = line_start + indent;
        let typed_words = if typed.is_empty() {
            [words.as_slice(), &[""]].concat()
        } else {
            words.clone()
        };
        let mut commands: Vec<Candidate> = statscraft_syntax::command_specs()
            .iter()
            .filter(|spec| name_matches(spec.name, &typed_words))
            .map(|spec| Candidate {
                text: spec.name.to_string(),
                detail: spec.summary.to_string(),
                what: What::Command,
            })
            .collect();
        commands.sort_by(|a, b| a.text.cmp(&b.text));
        // a macro call stands where a command does
        if typed_words.len() == 1 {
            commands.extend(macro_calls(typed_words[0]));
        }
        if !commands.is_empty() || words.len() <= 1 {
            return (commands, name_start..cursor);
        }
    }

    // inside a command: the command named on this line or carried from the lines above
    let mut state_here = state;
    highlight::line(&text[line_start..word_start], &mut state_here);
    let command = state_here.command.or(state.command);
    let mut out = Vec::new();
    let matches = |name: &str| starts_with_ignore_case(name, typed);
    let slash = text[..word_start].trim_end().ends_with('/');
    if let Some(command) = command {
        let (subcommands, keywords) = words_of(command);
        let list = if slash { subcommands } else { keywords };
        let what = if slash {
            What::Subcommand
        } else {
            What::Keyword
        };
        out.extend(list.iter().filter(|w| matches(w)).map(|w| Candidate {
            text: w.clone(),
            detail: String::new(),
            what,
        }));
    }
    if !slash {
        out.extend(macro_calls(typed).into_iter().filter(|_| !typed.is_empty()));
        if typed.starts_with('!') {
            out.extend(
                MACRO_WORDS
                    .iter()
                    .filter(|w| matches(w))
                    .map(|w| Candidate {
                        text: w.to_string(),
                        detail: String::new(),
                        what: What::Macro,
                    }),
            );
        }
        out.extend(
            variables
                .iter()
                .filter(|(name, _)| matches(name))
                .map(|(name, label)| Candidate {
                    text: name.clone(),
                    detail: label.clone(),
                    what: What::Variable,
                }),
        );
        if command.is_some_and(|c| EXPRESSIONS.contains(&c)) {
            out.extend(
                statscraft_engine::functions()
                    .iter()
                    .filter(|(name, _)| matches(name))
                    .map(|(name, form)| Candidate {
                        text: format!("{name}("),
                        detail: form.to_string(),
                        what: What::Function,
                    }),
            );
        }
    }
    // a word typed in full needs no completion
    if out.len() == 1 && out[0].text.eq_ignore_ascii_case(typed) {
        out.clear();
    }
    out.truncate(MOST);
    (out, word_start..cursor)
}

/// The supported command of that name.
pub fn spec(name: &str) -> Option<&'static statscraft_syntax::CommandSpec> {
    statscraft_syntax::command_specs()
        .iter()
        .find(|s| s.name == name)
}

/// The supported command whose name is written over byte `at`, for a card over it.
pub fn command_at_byte(text: &str, at: usize) -> Option<&'static statscraft_syntax::CommandSpec> {
    let (mut state, start) = highlight::state_at(text, at);
    let end = text[start..].find('\n').map_or(text.len(), |n| start + n);
    let line = text[start..end].trim_end_matches('\r');
    let (range, _) = highlight::line(line, &mut state)
        .into_iter()
        .find(|(r, k)| *k == highlight::Kind::Command && r.contains(&(at - start)))?;
    let words: Vec<&str> = line[range].split_whitespace().collect();
    spec(statscraft_syntax::command_name(&words)?.0)
}

/// Whether the typed words start the words of a command name: `VAL LAB` and `fre` match, and
/// so does a name with more words than typed.
fn name_matches(name: &str, typed: &[&str]) -> bool {
    let parts: Vec<&str> = name.split(' ').collect();
    !typed.is_empty()
        && typed.len() <= parts.len()
        && typed
            .iter()
            .zip(&parts)
            .all(|(t, p)| starts_with_ignore_case(p, t))
}

fn starts_with_ignore_case(text: &str, prefix: &str) -> bool {
    text.len() >= prefix.len()
        && text.is_char_boundary(prefix.len())
        && text[..prefix.len()].eq_ignore_ascii_case(prefix)
}

/// The text with a candidate accepted over `range`, and the byte where the cursor goes.
pub fn accept(text: &str, range: Range<usize>, candidate: &Candidate) -> (String, usize) {
    let mut out = String::with_capacity(text.len() + candidate.text.len());
    out.push_str(&text[..range.start]);
    out.push_str(&candidate.text);
    let mut cursor = out.len();
    let rest = &text[range.end..];
    // a command name or subcommand gets a blank after it, unless one follows already
    let space = candidate.what == What::Command && !rest.starts_with([' ', '\t', '.']);
    if space {
        out.push(' ');
        cursor += 1;
    }
    out.push_str(rest);
    (out, cursor)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars() -> Vec<(String, String)> {
        [("gender", "Gender"), ("age", "Age in years"), ("q1", "")]
            .iter()
            .map(|(n, l)| (n.to_string(), l.to_string()))
            .collect()
    }

    fn texts(text: &str, explicit: bool) -> (Vec<String>, Range<usize>) {
        let (list, range) = candidates(text, text.len(), &vars(), &[], explicit);
        (list.into_iter().map(|c| c.text).collect(), range)
    }

    #[test]
    fn command_names_complete_at_the_start_of_a_command() {
        let (list, range) = texts("GET FILE='a.sav'.\nFRE", false);
        assert_eq!(list, ["FREQUENCIES"]);
        assert_eq!(range, 18..21);
        // several words, each abbreviated
        assert_eq!(texts("val lab", false).0, ["VALUE LABELS"]);
        let (list, _) = texts("  VAR", false);
        assert!(list.contains(&"VARIABLE LABELS".to_string()));
        assert!(list.contains(&"VARIABLE LEVEL".to_string()));
        // nothing typed: only on request
        assert!(texts("", false).0.is_empty());
        assert!(texts("", true).0.len() > 30);
        // the line after a command without its period is still that command
        assert!(
            !texts("FREQUENCIES\n  ag", false)
                .0
                .contains(&"AGGREGATE".to_string())
        );
    }

    #[test]
    fn inside_a_command_variables_subcommands_and_functions() {
        let (list, range) = texts("fre gen", false);
        assert_eq!(list, ["gender"]);
        assert_eq!(range, 4..7);
        let (list, _) = texts("FREQUENCIES /VAR", false);
        assert_eq!(list, ["VARIABLES"]);
        let (list, _) = texts("FREQUENCIES\n  /", true);
        assert!(list.contains(&"VARIABLES".to_string()));
        // functions only where an expression goes
        let (list, _) = texts("COMPUTE x = me", false);
        assert_eq!(list, ["MEAN("]);
        assert!(texts("fre ab", false).0.is_empty());
        let (list, _) = texts("compute y = a", false);
        assert!(list.contains(&"age".to_string()));
        assert!(list.contains(&"ABS(".to_string()));
        // labels come along
        let (list, _) = candidates("fre a", 5, &vars(), &[], false);
        let age = list.iter().find(|c| c.text == "age").unwrap();
        assert_eq!(age.detail, "Age in years");
        // a word typed in full is left alone
        assert!(texts("fre age", false).0.is_empty());
    }

    #[test]
    fn macros_of_the_text_and_the_session_complete() {
        let text = "DEFINE !freq (!POSITIONAL !TOKENS(1)) FRE !1. !ENDDEFINE.\nDEFINE tab2 ()\n\
                    CROSSTABS a BY b.\n!ENDDEFINE.\n";
        let session = vec!["TableSet".to_string()];
        let at_end = |typed: &str| {
            let text = format!("{text}{typed}");
            let (list, _) = candidates(&text, text.len(), &vars(), &session, false);
            list.into_iter()
                .map(|c| (c.text, c.what))
                .collect::<Vec<_>>()
        };
        // where a command goes, beside the commands
        let list = at_end("t");
        assert!(list.contains(&("tab2".to_string(), What::Macro)));
        assert!(list.contains(&("TableSet".to_string(), What::Macro)));
        assert!(list.iter().any(|(_, w)| *w == What::Command));
        assert_eq!(at_end("!fr"), [("!freq".to_string(), What::Macro)]);
        // inside a command, with the words of the macro language after !
        let list = at_end("FREQUENCIES !");
        assert!(list.contains(&("!freq".to_string(), What::Macro)));
        assert!(list.contains(&("!ENDDEFINE".to_string(), What::Macro)));
        assert!(at_end("fre tab").contains(&("tab2".to_string(), What::Macro)));
    }

    #[test]
    fn commands_and_subcommands_of_the_table_complete() {
        let (list, _) = texts("FREQUENCIES age\n  /", true);
        for subcommand in ["FORMAT", "PERCENTILES", "NTILES", "STATISTICS", "MISSING"] {
            assert!(list.contains(&subcommand.to_string()), "{subcommand}");
        }
        let (list, _) = texts("RANK age\n  /", true);
        for subcommand in ["NTILES", "TIES", "FRACTION", "SAVAGE"] {
            assert!(list.contains(&subcommand.to_string()), "{subcommand}");
        }
        for (typed, command) in [
            ("ran", "RANK"),
            ("se", "SET"),
            ("dataset n", "DATASET NAME"),
            ("inc", "INCLUDE"),
            ("get da", "GET DATA"),
            ("autor", "AUTORECODE"),
        ] {
            assert!(
                texts(typed, false).0.contains(&command.to_string()),
                "{command}"
            );
        }
    }

    #[test]
    fn a_command_name_under_the_pointer_has_its_card() {
        let text = "GET FILE='a.sav'.\nval lab x 1 'a'.\n";
        assert_eq!(command_at_byte(text, 1).unwrap().name, "GET");
        assert_eq!(command_at_byte(text, 23).unwrap().name, "VALUE LABELS");
        // the body of a command and a line end are no name
        assert!(command_at_byte(text, 10).is_none());
        assert!(command_at_byte(text, 17).is_none());
        // recognised but unsupported commands have no card
        assert!(command_at_byte("DISCRIMINANT x.", 1).is_none());
    }

    #[test]
    fn accepting_replaces_what_was_typed() {
        let text = "fre\nexecute.";
        let (list, range) = candidates(text, 3, &vars(), &[], false);
        let (out, cursor) = accept(text, range, &list[0]);
        assert_eq!(out, "FREQUENCIES \nexecute.");
        assert_eq!(cursor, 12);
        let text = "fre ag.";
        let (list, range) = candidates(text, 6, &vars(), &[], false);
        assert_eq!(accept(text, range, &list[0]), ("fre age.".to_string(), 7));
    }
}
