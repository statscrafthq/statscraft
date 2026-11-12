//! Renders the command table to `docs/commands/` and fails when the pages there are out of date.
//! Regenerate with `UPDATE_DOCS=1 cargo test -p statscraft-syntax --test command_docs`.

use statscraft_syntax::{CommandSpec, command_specs, help, unsupported_commands};
use std::path::PathBuf;

/// The index groups every supported command under one area. A new command must be added here.
const AREAS: &[(&str, &[&str])] = &[
    (
        "Data files and dictionary",
        &[
            "GET",
            "GET DATA",
            "GET TRANSLATE",
            "IMPORT",
            "SYSFILE INFO",
            "SAVE",
            "SAVE TRANSLATE",
            "EXPORT",
            "XSAVE",
            "NEW FILE",
            "ADD FILES",
            "MATCH FILES",
            "STAR JOIN",
            "UPDATE",
            "VARIABLE LABELS",
            "VALUE LABELS",
            "MISSING VALUES",
            "VARIABLE LEVEL",
            "VARIABLE WIDTH",
            "FORMATS",
            "PRINT FORMATS",
            "WRITE FORMATS",
            "RENAME VARIABLES",
            "DELETE VARIABLES",
            "SORT CASES",
            "SORT VARIABLES",
            "WEIGHT",
            "AGGREGATE",
            "VARSTOCASES",
            "CASESTOVARS",
            "FLIP",
            "ADD VALUE LABELS",
            "FILE LABEL",
            "DOCUMENT",
            "ADD DOCUMENT",
            "DROP DOCUMENTS",
            "VARIABLE ROLE",
            "VARIABLE ALIGNMENT",
            "VARIABLE ATTRIBUTE",
            "DATAFILE ATTRIBUTE",
            "ALTER TYPE",
            "APPLY DICTIONARY",
            "DATA LIST",
            "BEGIN DATA",
            "MATRIX DATA",
            "INPUT PROGRAM",
            "END INPUT PROGRAM",
            "END CASE",
            "END FILE",
            "REREAD",
            "REPEATING DATA",
            "FILE TYPE",
            "RECORD TYPE",
            "END FILE TYPE",
            "SPLIT FILE",
            "FILTER",
            "TEMPORARY",
            "N OF CASES",
            "N",
            "SAMPLE",
        ],
    ),
    (
        "Transformations",
        &[
            "COMPUTE",
            "IF",
            "DO IF",
            "ELSE IF",
            "ELSE",
            "END IF",
            "RECODE",
            "COUNT",
            "SELECT IF",
            "STRING",
            "NUMERIC",
            "LEAVE",
            "EXECUTE",
            "DO REPEAT",
            "LOOP",
            "END LOOP",
            "BREAK",
            "VECTOR",
            "AUTORECODE",
            "RANK",
            "DATE",
            "PRINT",
            "PRINT EJECT",
            "PRINT SPACE",
            "WRITE",
            "CLEAR TRANSFORMATIONS",
        ],
    ),
    (
        "Procedures",
        &[
            "FREQUENCIES",
            "DESCRIPTIVES",
            "CROSSTABS",
            "MULT RESPONSE",
            "MEANS",
            "T-TEST",
            "ONEWAY",
            "CORRELATIONS",
            "NONPAR CORR",
            "PARTIAL CORR",
            "NPAR TESTS",
            "EXAMINE",
            "REGRESSION",
            "RELIABILITY",
            "FACTOR",
            "QUICK CLUSTER",
            "CLUSTER",
            "DISCRIMINANT",
            "LOGISTIC REGRESSION",
            "UNIANOVA",
            "GLM",
            "ANOVA",
            "TREE",
            "REPORT",
            "MATRIX",
        ],
    ),
    ("Custom tables", &["CTABLES", "MRSETS"]),
    (
        "Macros, files and session",
        &[
            "DEFINE",
            "INSERT",
            "INCLUDE",
            "BEGIN PROGRAM",
            "SET",
            "SHOW",
            "PRESERVE",
            "RESTORE",
            "CACHE",
            "CD",
            "FILE HANDLE",
            "DATASET NAME",
            "DATASET ACTIVATE",
            "DATASET COPY",
            "DATASET DECLARE",
            "DATASET CLOSE",
            "DATASET DISPLAY",
            "USE",
            "DISPLAY",
            "LIST",
            "TITLE",
            "SUBTITLE",
            "ECHO",
            "ERASE",
            "PERMISSIONS",
            "HOST",
            "OUTPUT NEW",
            "OUTPUT NAME",
            "OUTPUT ACTIVATE",
            "OUTPUT CLOSE",
            "OUTPUT DISPLAY",
            "OUTPUT EXPORT",
            "FINISH",
        ],
    ),
];

const REGENERATE: &str = "UPDATE_DOCS=1 cargo test -p statscraft-syntax --test command_docs";

fn file_name(command: &str) -> String {
    format!("{}.md", command.to_lowercase().replace(' ', "-"))
}

/// Escapes the characters Markdown would read as formatting, so the help text shows as written.
fn escape(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if "\\`*_<>[]#|~$".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    // a paragraph starting with "+ ", "- " or "1. " would become a list
    let digits = out.chars().take_while(char::is_ascii_digit).count();
    let marker = if out.starts_with("+ ") || out.starts_with("- ") {
        Some(0)
    } else if digits > 0 && (out[digits..].starts_with(". ") || out[digits..].starts_with(") ")) {
        Some(digits)
    } else {
        None
    };
    if let Some(at) = marker {
        out.insert(at, '\\');
    }
    out
}

/// The syntax text is the diagram (its first line, continuation lines that start with a space
/// and further forms that start with the command name), then note lines. A note paragraph ends
/// at a line that ends with a period.
fn page(spec: &CommandSpec) -> String {
    let lines: Vec<&str> = spec.syntax.lines().collect();
    let diagram = 1 + lines[1..]
        .iter()
        .take_while(|line| line.starts_with(' ') || line.starts_with(spec.name))
        .count();
    let mut notes = Vec::new();
    let mut paragraph = Vec::new();
    for line in &lines[diagram..] {
        paragraph.push(line.trim());
        if line.ends_with('.') {
            notes.push(escape(&paragraph.join(" ")));
            paragraph.clear();
        }
    }
    if !paragraph.is_empty() {
        notes.push(escape(&paragraph.join(" ")));
    }
    let mut out = format!(
        "<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->\n\n\
         # {}\n\n{}\n\n## Syntax\n\n```text\n{}\n```\n\n",
        spec.name,
        escape(spec.summary),
        lines[..diagram].join("\n"),
    );
    for note in notes {
        out.push_str(&note);
        out.push_str("\n\n");
    }
    out.push_str(&format!(
        "## Example\n\n```text\n{}\n```\n\n`statscraft help {}` prints the same text in the terminal. \
         [All commands](README.md)\n",
        spec.example, spec.name
    ));
    out
}

fn index() -> String {
    let mut out = String::from(
        "<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->\n\n\
         # Commands\n\n\
         One page per supported command, generated from the same table that the parser and \
         `statscraft help` use, so the pages always match the program.\n",
    );
    for (area, names) in AREAS {
        out.push_str(&format!(
            "\n## {area}\n\n| Command | What it does |\n|---|---|\n"
        ));
        for name in *names {
            let spec = command_specs().iter().find(|s| s.name == *name).unwrap();
            out.push_str(&format!(
                "| [{name}]({}) | {} |\n",
                file_name(name),
                escape(spec.summary)
            ));
        }
    }
    let general = help(None).unwrap();
    let (_, rules) = general.split_once("General rules:\n").unwrap();
    out.push_str(&format!(
        "\n## General rules of the language\n\n```text\n{}```\n",
        rules
    ));
    out.push_str(
        "\n## Not supported yet\n\nThese commands of the language are recognised, but Statscraft \
         does not run them yet. A script that uses one stops with the error \
         \"command ... is not supported yet\" at its line, and nothing is changed.\n\n",
    );
    out.push_str(&escape(&unsupported_commands().join(", ")));
    out.push('\n');
    out
}

/// Every generated page as (file name, content).
fn render() -> Vec<(String, String)> {
    let mut listed: Vec<&str> = AREAS
        .iter()
        .flat_map(|(_, names)| names.iter().copied())
        .collect();
    listed.sort();
    let mut names: Vec<&str> = command_specs().iter().map(|s| s.name).collect();
    names.sort();
    assert_eq!(
        listed, names,
        "every command must be listed once in AREAS of crates/syntax/tests/command_docs.rs"
    );
    let mut pages: Vec<(String, String)> = command_specs()
        .iter()
        .map(|spec| (file_name(spec.name), page(spec)))
        .collect();
    pages.push(("README.md".into(), index()));
    pages
}

#[test]
fn command_pages_are_up_to_date() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/commands");
    let pages = render();
    let on_disk: Vec<String> = std::fs::read_dir(&dir)
        .map(|entries| {
            entries
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .filter(|name| name.ends_with(".md"))
                .collect()
        })
        .unwrap_or_default();
    if std::env::var_os("UPDATE_DOCS").is_some() {
        std::fs::create_dir_all(&dir).unwrap();
        for name in &on_disk {
            std::fs::remove_file(dir.join(name)).unwrap();
        }
        for (name, text) in &pages {
            std::fs::write(dir.join(name), text).unwrap();
        }
        return;
    }
    let mut stale: Vec<&str> = pages
        .iter()
        .filter(|(name, text)| std::fs::read_to_string(dir.join(name)).ok().as_ref() != Some(text))
        .map(|(name, _)| name.as_str())
        .collect();
    stale.extend(
        on_disk
            .iter()
            .filter(|name| !pages.iter().any(|(page, _)| page == *name))
            .map(String::as_str),
    );
    assert!(
        stale.is_empty(),
        "docs/commands is out of date ({}); regenerate with: {REGENERATE}",
        stale.join(", ")
    );
}

#[test]
fn markdown_escapes_keep_the_text() {
    assert_eq!(escape("a * b_c <5 $q5"), "a \\* b\\_c \\<5 \\$q5");
    assert_eq!(escape("- x"), "\\- x");
    assert_eq!(escape("2. x"), "2\\. x");
    assert_eq!(escape("1.7 x"), "1.7 x");
}
