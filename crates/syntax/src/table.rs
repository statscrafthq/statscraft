//! The command table: the single source for command name matching, parsing and help.

use crate::ast::Command;
use crate::commands;
use crate::data_commands;
use crate::lexer::{Punct, Token, TokenKind};
use crate::parser::{PResult, Parser};

pub(crate) struct CommandSpec {
    pub name: &'static str,
    pub summary: &'static str,
    /// The syntax diagram, then optional note lines.
    pub syntax: &'static str,
    pub example: &'static str,
    pub parse: fn(&mut Parser) -> PResult<Command>,
}

pub(crate) static COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        name: "GET",
        summary: "Read a data file and make it the active dataset.",
        syntax: "GET FILE='path'.",
        example: "GET FILE='survey.sav'.",
        parse: commands::get,
    },
    CommandSpec {
        name: "SAVE",
        summary: "Write the active dataset to a data file.",
        syntax: "SAVE OUTFILE='path' [/COMPRESSED | /UNCOMPRESSED | /ZCOMPRESSED].",
        example: "SAVE OUTFILE='survey_clean.sav' /COMPRESSED.",
        parse: commands::save,
    },
    CommandSpec {
        name: "VARIABLE LABELS",
        summary: "Set the labels of variables.",
        syntax: "VARIABLE LABELS var 'label' [[/] var 'label' ...].",
        example: "VARIABLE LABELS age 'Age in years' / region 'Home region'.",
        parse: commands::variable_labels,
    },
    CommandSpec {
        name: "VALUE LABELS",
        summary: "Replace the value labels of variables.",
        syntax: "VALUE LABELS varlist value 'label' [value 'label' ...] [/ varlist ...].\n\
                 Values are numbers or quoted strings. A varlist with no labels removes its labels.",
        example: "VALUE LABELS gender 1 'Male' 2 'Female' / plan 'b' 'Basic' 'p' 'Premium'.",
        parse: commands::value_labels,
    },
    CommandSpec {
        name: "MISSING VALUES",
        summary: "Declare user-missing values.",
        syntax: "MISSING VALUES varlist (values) [[/] varlist (values) ...].\n\
                 values: up to three values, or one range with at most one extra value.\n\
                 range: low THRU high, LOWEST THRU high, low THRU HIGHEST (LO and HI also work).\n\
                 Empty parentheses clear the missing values.",
        example: "MISSING VALUES q1 q2 (98, 99) / income (LOWEST THRU -1, 999999) / plan ().",
        parse: commands::missing_values,
    },
    CommandSpec {
        name: "VARIABLE LEVEL",
        summary: "Set the measurement level of variables.",
        syntax: "VARIABLE LEVEL varlist (SCALE | ORDINAL | NOMINAL) [[/] varlist (...) ...].",
        example: "VARIABLE LEVEL age (SCALE) / q1 TO q5 (ORDINAL).",
        parse: commands::variable_level,
    },
    CommandSpec {
        name: "FORMATS",
        summary: "Set the display format of variables.",
        syntax: "FORMATS varlist (format) [[/] varlist (format) ...].\n\
                 format: type, width and optional decimals, for example F8.2, A20, DATE11.",
        example: "FORMATS income (COMMA10.2) / age (F3).",
        parse: commands::formats,
    },
    CommandSpec {
        name: "FREQUENCIES",
        summary: "Frequency tables.",
        syntax: "FREQUENCIES [/]VARIABLES=varlist.\n\
                 Other subcommands are not supported yet.",
        example: "FREQUENCIES VARIABLES=gender region.",
        parse: commands::frequencies,
    },
    CommandSpec {
        name: "EXECUTE",
        summary: "Run the pending transformations.",
        syntax: "EXECUTE.",
        example: "EXECUTE.",
        parse: commands::execute,
    },
    CommandSpec {
        name: "COUNT",
        summary: "Count, per case, how many variables hold one of the listed values.",
        syntax: "COUNT target = varlist (values) [varlist (values) ...] [/ target = ...].\n\
                 values: values, low THRU high (with LOWEST or HIGHEST), MISSING, SYSMIS.\n\
                 A value or range counts user-missing values too; a string variable takes quoted values only.",
        example: "COUNT channels = q5_1 TO q5_5 (1) / unanswered = q1 q2 (MISSING).",
        parse: data_commands::count,
    },
    CommandSpec {
        name: "SORT CASES",
        summary: "Sort the cases by the values of variables.",
        syntax: "SORT CASES [BY] varlist [(A | D)] [varlist [(A | D)] ...].\n\
                 A is ascending (the default), D descending; a direction covers the variables before it.\n\
                 System-missing sorts lowest. Cases with equal keys keep their order.",
        example: "SORT CASES BY region (A) age (D).",
        parse: data_commands::sort_cases,
    },
    CommandSpec {
        name: "RENAME VARIABLES",
        summary: "Change the names of variables.",
        syntax: "RENAME VARIABLES (oldlist = newlist) [(oldlist = newlist) ...].\n\
                 RENAME VARIABLES old = new [old = new ...].\n\
                 All renames happen at once, so (a b = b a) swaps two names. new1 TO new3 counts up.",
        example: "RENAME VARIABLES (q1 q2 = satisfaction recommend) (v1 TO v3 = item1 TO item3).",
        parse: data_commands::rename_variables,
    },
    CommandSpec {
        name: "DELETE VARIABLES",
        summary: "Remove variables from the dataset.",
        syntax: "DELETE VARIABLES varlist.",
        example: "DELETE VARIABLES tmp1 tmp2 q7_1 TO q7_9.",
        parse: data_commands::delete_variables,
    },
    CommandSpec {
        name: "WEIGHT",
        summary: "Weight the cases by a variable, or turn weighting off.",
        syntax: "WEIGHT BY var.\n\
                 WEIGHT OFF.",
        example: "WEIGHT BY weight.",
        parse: data_commands::weight,
    },
    CommandSpec {
        name: "AGGREGATE",
        summary: "Aggregate groups of cases into new variables or a new dataset.",
        syntax: "AGGREGATE /OUTFILE=* | 'path' [MODE=ADDVARIABLES | REPLACE] [/BREAK=varlist [(A | D)] ...]\n\
                 \x20         /target ['label'] [target ...] = function(varlist) [/target ...].\n\
                 OUTFILE=* replaces the active dataset with one case per break group (break variables, then\n\
                 targets); with MODE=ADDVARIABLES it keeps the cases and adds the targets to each of them.\n\
                 OUTFILE='path' writes the aggregated dataset to a file and leaves the active dataset alone.\n\
                 functions: SUM MEAN SD MEDIAN MIN MAX FIRST LAST N NU NMISS NUMISS; N and NU also without (varlist).\n\
                 Missing values are skipped. With a weight variable SUM, MEAN, SD, MEDIAN, N and NMISS are weighted.",
        example: "AGGREGATE /OUTFILE=* MODE=ADDVARIABLES /BREAK=region /region_mean = MEAN(income) /region_n = N.",
        parse: data_commands::aggregate,
    },
    CommandSpec {
        name: "COMPUTE",
        summary: "Assign the value of an expression to a variable.",
        syntax: "COMPUTE target = expression.",
        example: "COMPUTE bmi = weight / (height / 100) ** 2.",
        parse: commands::compute,
    },
    CommandSpec {
        name: "IF",
        summary: "Assign a value only for the cases where a condition holds.",
        syntax: "IF (condition) target = expression.\n\
                 The parentheses around the condition are optional.",
        example: "IF (age >= 65 AND region = 2) senior_south = 1.",
        parse: commands::if_command,
    },
    CommandSpec {
        name: "SELECT IF",
        summary: "Keep only the cases where a condition holds.",
        syntax: "SELECT IF condition.",
        example: "SELECT IF age >= 18 AND NOT MISSING(q1).",
        parse: commands::select_if,
    },
    CommandSpec {
        name: "RECODE",
        summary: "Map values of variables to new values.",
        syntax: "RECODE varlist (inputs = output) [(inputs = output) ...] [INTO newvars] [/ varlist ...].\n\
                 inputs: values, low THRU high (with LOWEST or HIGHEST), MISSING, SYSMIS, ELSE.\n\
                 output: a value, SYSMIS or COPY.\n\
                 The first matching (inputs = output) wins. A value or range also matches user-missing values.\n\
                 Without a match a variable keeps its value; with INTO the target becomes system-missing or blank.\n\
                 A string target of INTO must exist already.",
        example: "RECODE age (LOWEST THRU 29 = 1) (30 THRU 59 = 2) (60 THRU HIGHEST = 3) INTO age_group.",
        parse: commands::recode,
    },
];

/// Commands of the language that have no parser yet. They get "not supported yet" instead of
/// "unknown command", and their bodies are not tokenised.
static UNSUPPORTED: &[&str] = &[
    "ADD DOCUMENT",
    "ADD FILES",
    "ADD VALUE LABELS",
    "ALTER TYPE",
    "ANOVA",
    "APPLY DICTIONARY",
    "AUTORECODE",
    "BEGIN DATA",
    "BREAK",
    "CACHE",
    "CASESTOVARS",
    "CD",
    "CLEAR TRANSFORMATIONS",
    "CLUSTER",
    "CORRELATIONS",
    "CROSSTABS",
    "CTABLES",
    "DATA LIST",
    "DATASET ACTIVATE",
    "DATASET CLOSE",
    "DATASET COPY",
    "DATASET DECLARE",
    "DATASET DISPLAY",
    "DATASET NAME",
    "DATE",
    "DEFINE",
    "DESCRIPTIVES",
    "DISCRIMINANT",
    "DISPLAY",
    "DO IF",
    "DO REPEAT",
    "DOCUMENT",
    "DROP DOCUMENTS",
    "ECHO",
    "ELSE",
    "ELSE IF",
    "END CASE",
    "END DATA",
    "END FILE",
    "END FILE TYPE",
    "END IF",
    "END INPUT PROGRAM",
    "END LOOP",
    "END MATRIX",
    "END REPEAT",
    "ERASE",
    "EXAMINE",
    "EXPORT",
    "FACTOR",
    "FILE HANDLE",
    "FILE LABEL",
    "FILE TYPE",
    "FILTER",
    "FINISH",
    "FLIP",
    "GET DATA",
    "GET TRANSLATE",
    "GLM",
    "GRAPH",
    "HOST",
    "IMPORT",
    "INCLUDE",
    "INPUT PROGRAM",
    "INSERT",
    "LEAVE",
    "LIST",
    "LOGISTIC REGRESSION",
    "LOOP",
    "MATCH FILES",
    "MATRIX",
    "MATRIX DATA",
    "MEANS",
    "MRSETS",
    "MULT RESPONSE",
    "N OF CASES",
    "NEW FILE",
    "NONPAR CORR",
    "NPAR TESTS",
    "NUMERIC",
    "ONEWAY",
    "OUTPUT CLOSE",
    "OUTPUT EXPORT",
    "OUTPUT NEW",
    "OUTPUT SAVE",
    "PARTIAL CORR",
    "PERMISSIONS",
    "PRESERVE",
    "PRINT",
    "PRINT EJECT",
    "PRINT FORMATS",
    "PRINT SPACE",
    "QUICK CLUSTER",
    "RANK",
    "RECORD TYPE",
    "REGRESSION",
    "RELIABILITY",
    "REPEATING DATA",
    "REPORT",
    "REREAD",
    "RESTORE",
    "SAMPLE",
    "SAVE TRANSLATE",
    "SET",
    "SHOW",
    "SORT VARIABLES",
    "SPLIT FILE",
    "STRING",
    "SUBTITLE",
    "SYSFILE INFO",
    "T-TEST",
    "TEMPORARY",
    "TITLE",
    "UNIANOVA",
    "UPDATE",
    "USE",
    "VARIABLE ALIGNMENT",
    "VARIABLE ATTRIBUTE",
    "VARIABLE ROLE",
    "VARIABLE WIDTH",
    "VARSTOCASES",
    "VECTOR",
    "WRITE",
    "WRITE FORMATS",
    "XSAVE",
];

fn names() -> impl Iterator<Item = (&'static str, Option<&'static CommandSpec>)> {
    let supported = COMMANDS.iter().map(|spec| (spec.name, Some(spec)));
    supported.chain(UNSUPPORTED.iter().map(|&name| (name, None)))
}

/// True when `word` is `keyword` or an abbreviation of it with at least three letters.
pub(crate) fn matches_keyword(word: &str, keyword: &str) -> bool {
    let (word, keyword) = (word.as_bytes(), keyword.as_bytes());
    let abbreviated = word.len() >= 3 && word.len() <= keyword.len();
    word.eq_ignore_ascii_case(keyword)
        || (abbreviated && keyword[..word.len()].eq_ignore_ascii_case(word))
}

/// The usize is the number of tokens (for `resolve`: words) that form the command name.
pub(crate) enum Resolution {
    Supported(&'static CommandSpec, usize),
    Unsupported(&'static str, usize),
    Ambiguous(Vec<&'static str>),
    Unknown,
}

/// Finds the command whose name words match the leading `words`. The longest name wins, and a
/// name written in full wins over an abbreviation.
pub(crate) fn resolve(words: &[&str]) -> Resolution {
    let mut best = Vec::new();
    let mut best_rank = (0, false);
    for (name, spec) in names() {
        let parts: Vec<&str> = name.split(' ').collect();
        let pairs = || parts.iter().zip(words);
        if parts.len() > words.len() || !pairs().all(|(part, word)| matches_keyword(word, part)) {
            continue;
        }
        let exact = pairs().all(|(part, word)| word.eq_ignore_ascii_case(part));
        let rank = (parts.len(), exact);
        if rank > best_rank {
            best.clear();
            best_rank = rank;
        }
        if rank == best_rank {
            best.push((name, spec));
        }
    }
    match best.as_slice() {
        [] => Resolution::Unknown,
        [(_, Some(spec))] => Resolution::Supported(spec, best_rank.0),
        [(name, None)] => Resolution::Unsupported(name, best_rank.0),
        many => Resolution::Ambiguous(many.iter().map(|(name, _)| *name).collect()),
    }
}

/// The leading words of a command, each with the number of tokens up to and including it.
/// `T-TEST` arrives as three tokens and is joined into one word here.
pub(crate) fn name_words(tokens: &[Token]) -> Vec<(String, usize)> {
    let word = |i: usize| match tokens.get(i).map(|t| &t.kind) {
        Some(TokenKind::Word(w)) => Some(w),
        _ => None,
    };
    let mut words = Vec::new();
    let mut i = 0;
    while let Some(w) = word(i) {
        let minus = tokens.get(i + 1).map(|t| &t.kind) == Some(&TokenKind::Punct(Punct::Minus));
        if let Some(test) = word(i + 2)
            && minus
            && w.eq_ignore_ascii_case("T")
            && test.eq_ignore_ascii_case("TEST")
        {
            i += 3;
            words.push(("T-TEST".to_string(), i));
        } else {
            i += 1;
            words.push((w.clone(), i));
        }
    }
    words
}

pub(crate) fn resolve_tokens(tokens: &[Token]) -> Resolution {
    let words = name_words(tokens);
    let strs: Vec<&str> = words.iter().map(|(w, _)| w.as_str()).collect();
    match resolve(&strs) {
        Resolution::Supported(spec, n) => Resolution::Supported(spec, words[n - 1].1),
        Resolution::Unsupported(name, n) => Resolution::Unsupported(name, words[n - 1].1),
        other => other,
    }
}

fn levenshtein(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let substitute = diagonal + usize::from(ca != *cb);
            diagonal = row[j + 1];
            row[j + 1] = substitute.min(diagonal + 1).min(row[j] + 1);
        }
    }
    row[b.len()]
}

/// Command names close to the unknown name that starts with `words` (not empty).
pub(crate) fn suggest(words: &[&str]) -> Vec<&'static str> {
    let upper: Vec<String> = words.iter().map(|w| w.to_uppercase()).collect();
    let typed =
        |from: usize, to: usize| upper[from.min(upper.len())..to.min(upper.len())].join(" ");
    let close = |typed: String, target: &str| {
        let distance = levenshtein(&typed, target);
        (distance <= (target.len() / 3).max(1)).then_some(distance)
    };
    // multi-word names that share the typed first word: the mistake is in a later word
    let family: Vec<(&str, &str)> = names()
        .filter_map(|(name, _)| {
            name.split_once(' ')
                .map(|(first, rest)| (name, first, rest))
        })
        .filter(|(_, first, _)| matches_keyword(words[0], first))
        .map(|(name, _, rest)| (name, rest))
        .collect();
    let mut scored: Vec<(usize, &str)> = if family.is_empty() {
        names()
            .filter_map(|(name, _)| {
                close(typed(0, name.split(' ').count()), name).map(|d| (d, name))
            })
            .collect()
    } else {
        family
            .iter()
            .filter_map(|(name, rest)| {
                close(typed(1, name.split(' ').count()), rest).map(|d| (d, *name))
            })
            .collect()
    };
    scored.sort();
    let best = scored.first().map(|(d, _)| *d);
    let near: Vec<&str> = scored
        .into_iter()
        .take_while(|(d, _)| Some(*d) == best)
        .map(|(_, name)| name)
        .take(3)
        .collect();
    if near.is_empty() {
        family.into_iter().map(|(name, _)| name).take(6).collect()
    } else {
        near
    }
}

/// The names of the commands that can be parsed, in help order.
pub fn command_names() -> Vec<&'static str> {
    COMMANDS.iter().map(|spec| spec.name).collect()
}

/// Help text, generated from the table the parser uses. `help(None)` lists the supported
/// commands with one-line summaries and the general rules of the language. A command name may
/// be abbreviated like in syntax. None for a name that is no command of the language.
pub fn help(command: Option<&str>) -> Option<String> {
    let Some(command) = command else {
        let mut text = String::from("Supported commands:\n");
        for spec in COMMANDS {
            text.push_str(&format!("  {:<18}{}\n", spec.name, spec.summary));
        }
        text.push_str(GENERAL_HELP);
        return Some(text);
    };
    let words: Vec<&str> = command.split_whitespace().collect();
    match resolve(&words) {
        Resolution::Supported(spec, used) if used == words.len() => {
            let syntax = spec.syntax.replace('\n', "\n  ");
            Some(format!(
                "{}: {}\n\nSyntax:\n  {}\n\nExample:\n  {}\n",
                spec.name, spec.summary, syntax, spec.example
            ))
        }
        Resolution::Unsupported(name, used) if used == words.len() => Some(format!(
            "{name} is a command of the language, but it is not supported yet.\n"
        )),
        _ => None,
    }
}

const GENERAL_HELP: &str = "
General rules:
  A command ends with a period at the end of a line, at a blank line, or at the end of input.
  Command names and keywords are case-insensitive and may be shortened to three or more letters.
  varlist: variable names, a range written as first TO last, or ALL.
  Strings use single or double quotes; double the quote character to include it.
  Comments: a command starting with * or COMMENT, or /* text */ inside a line.
  Expressions: + - * / **, = <> < > <= >= (or EQ NE LT GT LE GE), AND OR NOT (or & | ~),
  functions such as MEAN(a, b) or SUM(q1 TO q5).
";
