//! Strings over line breaks, free-text commands, macros and inserted files.

use super::*;

fn one(source: &str) -> Command {
    let (mut parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    assert_eq!(parsed.len(), 1, "{source}");
    parsed.remove(0).command
}

fn layout(source: &str) -> Vec<(String, u32, u32)> {
    split_commands(source)
        .unwrap()
        .into_iter()
        .map(|c| (c.name, c.first_line, c.last_line))
        .collect()
}

fn names(source: &str) -> Vec<(String, u32, u32)> {
    layout(source)
}

fn rows(expected: &[(&str, u32, u32)]) -> Vec<(String, u32, u32)> {
    expected
        .iter()
        .map(|&(name, first, last)| (name.to_string(), first, last))
        .collect()
}

#[test]
fn a_string_runs_on_over_line_breaks_and_blank_lines_until_its_quote() {
    let source = "VARIABLE LABELS q1 \"First line.\n\nThird line\"\n  /q2 'Two.\nlines'.\nEXECUTE.";
    assert_eq!(
        one(&source[..source.len() - 9]),
        Command::VariableLabels(vec![
            ("q1".into(), "First line.\n\nThird line".into()),
            ("q2".into(), "Two.\nlines".into()),
        ])
    );
    assert_eq!(
        names(source),
        rows(&[("VARIABLE LABELS", 1, 5), ("EXECUTE", 6, 6)])
    );
    // segments joined with + over several lines, each segment spanning lines itself
    assert_eq!(
        one("VALUE LABELS q1 1 'a\nb'+\n'c' 2 \"d\".\n"),
        Command::ValueLabels(vec![ValueLabels {
            vars: vec![VarItem::Name("q1".into())],
            labels: vec![
                (Value::Number(1.0), "a\nbc".into()),
                (Value::Number(2.0), "d".into())
            ],
        }])
    );
}

#[test]
fn a_quote_in_free_text_or_an_unknown_command_stays_on_its_line() {
    let source = "TITLE Don't panic.\nBOGUS it's 'open.\nEXECUTE.";
    assert_eq!(
        names(source),
        rows(&[("TITLE", 1, 1), ("BOGUS", 2, 2), ("EXECUTE", 3, 3)])
    );
}
