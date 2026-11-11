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

/// The lines that come out of macro expansion, blank ones left out, as "number: text".
fn expanded(source: &str) -> Vec<String> {
    preprocess::preprocess(source, None)
        .into_iter()
        .filter_map(|piece| match piece {
            preprocess::Piece::Line(line) if !line.text.trim().is_empty() => {
                Some(format!("{}: {}", line.number, line.text.trim()))
            }
            preprocess::Piece::Line(_) => None,
            preprocess::Piece::Error(e) => Some(format!("error {e}")),
        })
        .collect()
}

/// The commands after expansion, as their names; errors as "error ...".
fn commands(source: &str) -> Vec<String> {
    let (parsed, errors) = parse(source);
    let mut out: Vec<String> = parsed.iter().map(|p| format!("{:?}", p.command)).collect();
    out.extend(errors.iter().map(|e| format!("error {e}")));
    out
}

#[test]
fn positional_arguments_take_tokens_up_to_a_character_or_the_end_of_the_command() {
    let source = "define !two (!positional !tokens(1) / !positional !charend('/')\n\
                  / !positional !enclose('(',')') / !positional !cmdend)\n\
                  FREQUENCIES VARIABLES=!1 !2 !3 !4.\n\
                  !enddefine.\n\
                  !two a b c / (d e) f g.";
    assert_eq!(
        expanded(source),
        [
            "1: DEFINE '!TWO'",
            "4: .",
            "5: FREQUENCIES VARIABLES=a b c d e f g.",
            "5: ."
        ]
    );
    // !* is every positional value; a value left out is empty
    let source = "DEFINE !all (!POS !TOKENS(2) / !POS !CHAREND('-')) DESCRIPTIVES !*. !ENDDEFINE.\n!all x y z -.\n!all x.";
    assert_eq!(
        expanded(source),
        [
            "1: DEFINE '!ALL'.",
            "2: DESCRIPTIVES x y z.",
            "2: .",
            "3: DESCRIPTIVES x.",
            "3: ."
        ]
    );
}

#[test]
fn keyword_arguments_come_in_any_order_after_the_positional_ones_and_have_defaults() {
    let source = "DEFINE stats (!POSITIONAL !TOKENS(1) / vars = !CHAREND('/') / stat = !DEFAULT(MEAN) !TOKENS(1))\n\
                  DESCRIPTIVES !vars /STATISTICS=!stat !1.\n\
                  !ENDDEFINE.\n\
                  stats SUM stat = MAX vars = a b /.\n\
                  STATS min vars=c/.";
    assert_eq!(
        expanded(source)[2..],
        [
            "4: DESCRIPTIVES a b /STATISTICS=MAX SUM.",
            "4: .",
            "5: DESCRIPTIVES c /STATISTICS=MEAN min.",
            "5: ."
        ]
    );
}

#[test]
fn tokens_no_argument_takes_stay_in_the_command() {
    let source =
        "DEFINE !v (!POSITIONAL !CHAREND('/')) FREQUENCIES VARIABLES=!1 !ENDDEFINE.\n!v a b / c.";
    assert_eq!(expanded(source)[1..], ["2: FREQUENCIES VARIABLES=a b c."]);
}

#[test]
fn let_if_else_and_loops_build_the_expansion() {
    let source = "DEFINE !m (!POSITIONAL !CMDEND)\n\
                  !LET !n = !LENGTH(!1)\n\
                  !IF (!n > 3) !THEN long !ELSE short !IFEND\n\
                  !DO !i = 1 !TO 7 !BY 3 v!i !DOEND\n\
                  !DO !w !IN (!1) !IF (!w = 'c') !THEN !BREAK !IFEND !UPCASE(!w) !DOEND\n\
                  !IF (!NOT (!1 = 'x') !AND 1 <> 2 | 0) !THEN yes !IFEND\n\
                  !ENDDEFINE.\n\
                  !m a b c.";
    let lines: Vec<String> = expanded(source)
        .into_iter()
        .filter(|l| l.starts_with("8:"))
        .collect();
    // the body is read in upper case; arguments keep theirs
    assert_eq!(lines, ["8: LONG", "8: V1 V4 V7", "8: A B", "8: YES."]);
}

#[test]
fn numbers_compare_as_numbers_and_text_as_text_with_case() {
    let source = "DEFINE !c (!POSITIONAL !TOKENS(1) / !POSITIONAL !TOKENS(1))\n\
                  !IF (!1 < !2) !THEN lt !ELSE ge !IFEND\n\
                  !ENDDEFINE.\n\
                  !c 9 10.\n!c b A.\n!c '1' 1.0.";
    assert_eq!(expanded(source)[2..], ["4: LT.", "5: GE.", "6: GE."]);
}

#[test]
fn string_functions_work_as_documented() {
    // the body is read in upper case, so lower case survives only inside quotes
    let cases = [
        ("!LENGTH(abcdef)", "6"),
        ("!LENGTH('abcdef')", "8"),
        ("!CONCAT(x, 'y', 1)", "Xy1"),
        ("!SUBSTR(banana, 3, 2)", "NA"),
        ("!SUBSTR(banana, 3)", "NANA"),
        ("!INDEX(banana, nan)", "3"),
        ("!INDEX(banana, kiwi)", "0"),
        ("!HEAD('a b c')", "a"),
        ("!TAIL('a b c')", "b c"),
        ("!QUOTE(abc)", "'ABC'"),
        ("!QUOTE('abc')", "'abc'"),
        ("!QUOTE(!UNQUOTE(\"Bill's\"))", "'Bill''s'"),
        ("!UNQUOTE('it''s')", "it's"),
        ("!UPCASE('a b')", "A B"),
        ("!QUOTE(!BLANKS(3))", "'   '"),
        ("!LENGTH(!NULL)", "0"),
        ("!QUOTE(!NULL)", "''"),
        ("!CONCAT(!NULL, 123)", "123"),
    ];
    for (call, expected) in cases {
        let source = format!("DEFINE !f () !LET !r = {call} [!r] !ENDDEFINE.\nTITLE !f.");
        assert_eq!(
            expanded(&source)[1],
            format!("2: TITLE [{expected}]."),
            "{call}"
        );
    }
}
#[test]
fn eval_expands_calls_inside_a_function_argument() {
    let source = "DEFINE !vars () a b c !ENDDEFINE.\n\
                  DEFINE !n () !LENGTH(!vars) !LENGTH(!EVAL(!vars)) !ENDDEFINE.\n\
                  TITLE !n.";
    assert_eq!(expanded(source)[2..], ["3: TITLE 5 5."]);
}

#[test]
fn macros_call_macros_and_a_body_period_ends_only_a_whole_command() {
    let source = "define !caption()\n  'Table'.\n!enddefine.\n\
                  define !inner (!positional !cmdend) FREQUENCIES VARIABLES=!1. !enddefine.\n\
                  DEFINE outer (!POSITIONAL !CHAREND('-') / !POSITIONAL !CMDEND)\n\
                  !inner !1.\n\
                  CTABLES /TABLE !2 /TITLES TITLE=!caption CAPTION=!caption\n  /SLABELS VISIBLE=NO.\n\
                  !ENDDEFINE.\n\
                  outer q1 - q2.";
    let lines: Vec<String> = expanded(source)
        .into_iter()
        .filter(|l| l.starts_with("10:"))
        .collect();
    // the title macro's period is dropped in the middle of CTABLES; the inner call ends
    // its own command, whose period stands on a line of its own then
    assert_eq!(
        lines,
        [
            "10: FREQUENCIES VARIABLES=q1.",
            "10: .",
            "10: CTABLES /TABLE q2 /TITLES TITLE='Table' CAPTION='Table'",
            "10: /SLABELS VISIBLE=NO.",
            "10: ."
        ]
    );
    let parsed = commands(source);
    assert_eq!(parsed.len(), 5, "{parsed:?}");
    assert!(parsed.iter().all(|c| !c.starts_with("error")), "{parsed:?}");
}

#[test]
fn offexpand_and_mexpand_stop_expansion() {
    let source = "DEFINE !x () XX !ENDDEFINE.\n\
                  DEFINE !y () !OFFEXPAND !x !ONEXPAND !x !ENDDEFINE.\n\
                  TITLE !y.\n\
                  SET MEXPAND=OFF.\n\
                  TITLE !x.\n\
                  SET MEXPAND ON.\n\
                  TITLE !x.";
    assert_eq!(
        expanded(source),
        [
            "1: DEFINE '!X'.",
            "2: DEFINE '!Y'.",
            "3: TITLE !X XX.",
            "4: SET MEXPAND=OFF.",
            "5: TITLE !x.",
            "6: SET MEXPAND ON.",
            "7: TITLE XX."
        ]
    );
}

#[test]
fn arguments_are_expanded_when_the_macro_is_called_unless_noexpand() {
    let source = "DEFINE !seven () 7 !ENDDEFINE.\n\
                  DEFINE !plain (a = !TOKENS(1)) !a !ENDDEFINE.\n\
                  DEFINE !kept (a = !NOEXPAND !TOKENS(1)) !a !ENDDEFINE.\n\
                  DEFINE !len (a = !TOKENS(1)) !LENGTH(!a) !ENDDEFINE.\n\
                  DEFINE !lenkept (a = !NOEXPAND !TOKENS(1)) !LENGTH(!a) !ENDDEFINE.\n\
                  TITLE !plain a=!seven.\n\
                  TITLE !kept a=!seven.\n\
                  TITLE !len a=!seven.\n\
                  TITLE !lenkept a=!seven.";
    let lines: Vec<String> = expanded(source).into_iter().skip(5).collect();
    // functions see the expanded value; a !NOEXPAND one stays a macro name to the end
    assert_eq!(
        lines,
        [
            "6: TITLE 7.",
            "7: TITLE !seven.",
            "8: TITLE 1.",
            "9: TITLE 6."
        ]
    );
    // a variable list kept in a macro, its body ending with a period, passed to a macro that
    // walks it with !HEAD and !TAIL
    let source = "DEFINE !vars () a b c.\n!ENDDEFINE.\n\
                  DEFINE !sum (!POSITIONAL !CMDEND) !LET !s = !HEAD(!1) !LET !r = !TAIL(!1)\n\
                  !DO !i = 1 !TO 10 !IF (!r = !NULL) !THEN !BREAK !IFEND\n\
                  !LET !s = !CONCAT(!s, ' + ', !HEAD(!r)) !LET !r = !TAIL(!r) !DOEND !s !ENDDEFINE.\n\
                  COMPUTE t = !sum !vars.";
    assert_eq!(
        expanded(source).last().unwrap(),
        "6: COMPUTE t = A + B + C."
    );
    // the syntax chart writes !POSITIONAL=, the examples leave the = out
    let source = "DEFINE !two (!POSITIONAL = !TOKENS(1) / !POSITIONAL !TOKENS(1)) !2 !1 !ENDDEFINE.\n\
                  TITLE !two a b.";
    assert_eq!(expanded(source).last().unwrap(), "2: TITLE b a.");
}

#[test]
fn mprint_shows_the_expanded_command() {
    let source = "DEFINE !v () a b !ENDDEFINE.\nSET MPRINT=YES.\nFREQUENCIES VARIABLES=!v.";
    let raw = split_commands(source).unwrap();
    // the body is read in upper case
    assert_eq!(raw[2].echo.as_deref(), Some("FREQUENCIES VARIABLES=A B"));
    assert_eq!(raw[2].first_line, 3);
    assert!(raw[1].echo.is_none());
}

#[test]
fn errors_in_an_expansion_point_at_the_call_and_name_the_macro() {
    let source = "DEFINE !bad () VARIABLE LEVEL x (HUGE) !ENDDEFINE.\nEXECUTE.\n!bad.";
    let (_, errors) = parse(source);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].span.line, 3);
    assert_eq!(
        errors[0].hint.as_deref(),
        Some("in the expansion of macro !BAD")
    );
    let errors = commands("DEFINE !m (!POSITIONAL !TOKENS(0)) x !ENDDEFINE.");
    assert_eq!(
        errors,
        ["error line 1, column 32: !TOKENS needs a positive whole number"]
    );
    let errors = commands("DEFINE !m () !IF (1) !THEN x !ENDDEFINE.\n!m.");
    assert_eq!(
        errors,
        [
            "Define(\"!M\")",
            "error line 2, column 1: in macro !M: !IF without !IFEND"
        ]
    );
    let errors = commands("DEFINE !m () x\nEXECUTE.");
    assert_eq!(
        errors,
        ["error line 1, column 1: DEFINE without !ENDDEFINE"]
    );
    let errors = commands("DEFINE !m () !m !ENDDEFINE.\nTITLE !m.");
    assert!(
        errors[1].contains("nested more than 50 levels"),
        "{errors:?}"
    );
    let errors = commands("DEFINE !m () !DO !i = 1 !TO 5000 x !DOEND !ENDDEFINE.\nTITLE !m.");
    assert!(errors[1].contains("more than 1000 times"), "{errors:?}");
}

#[test]
fn program_blocks_are_set_aside_and_data_blocks_pass_unchanged() {
    let source = "BEGIN PROGRAM PYTHON3.\nimport os\nx = 'it''s'\nEND PROGRAM.\nEXECUTE.";
    assert_eq!(
        layout(source),
        rows(&[("BEGIN PROGRAM", 1, 4), ("EXECUTE", 5, 5)])
    );
    assert_eq!(commands(source), ["BeginProgram", "Execute"]);
    let source = "DEFINE !d () 7 !ENDDEFINE.\nBEGIN DATA\n!d 'x\nEND DATA.";
    assert_eq!(
        expanded(source)[1..],
        ["2: BEGIN DATA", "3: !d 'x", "4: END DATA."]
    );
}

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("statscraft-pre-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("lib")).unwrap();
    dir
}

#[test]
fn insert_and_include_read_files_relative_to_the_including_file() {
    let dir = temp_dir("insert");
    // the library defines a macro by the batch rules: continuation lines are indented
    std::fs::write(
        dir.join("lib/defs.sps"),
        "DEFINE !freq (!POSITIONAL !CMDEND)\n FREQUENCIES VARIABLES=!1.\n!ENDDEFINE\nINSERT FILE='more.sps'.\n",
    )
    .unwrap();
    // windows-1257: 0xF0 is s with caron
    std::fs::write(dir.join("lib/more.sps"), b"VARIABLE LABELS x '\xF0'.\n").unwrap();
    let main = "INCLUDE FILE='lib/defs.sps' ENCODING='1257'.\n!freq a b.";
    let raw = split_commands_in(main, Some(&dir)).unwrap();
    let names: Vec<(&str, u32, Option<&str>)> = raw
        .iter()
        .map(|c| (c.name.as_str(), c.first_line, c.origin.as_deref()))
        .collect();
    assert_eq!(
        names,
        [
            ("INCLUDE", 1, None),
            ("DEFINE", 1, Some("lib/defs.sps, line 1")),
            ("INSERT", 1, Some("lib/defs.sps, line 4")),
            (
                "VARIABLE LABELS",
                1,
                Some("more.sps (from lib/defs.sps, line 4), line 1")
            ),
            ("FREQUENCIES", 2, Some("the expansion of macro !FREQ")),
        ]
    );
    // more.sps is read as UTF-8 since it names no encoding and the byte is not UTF-8:
    // the legacy code page decodes it to one character
    let label = &raw[3].tokens[3];
    assert!(matches!(&label.kind, TokenKind::Str(s) if s.chars().count() == 1));
    // a file that inserts itself is an error; a missing one is left to the run
    std::fs::write(dir.join("loop.sps"), "INSERT FILE='loop.sps'.\n").unwrap();
    let error = split_commands_in("INSERT FILE='loop.sps'.", Some(&dir)).unwrap_err();
    assert_eq!(error.message, "loop.sps inserts itself");
    let raw = split_commands_in("INSERT FILE='none.sps'.\nEXECUTE.", Some(&dir)).unwrap();
    assert_eq!(raw.len(), 2);
    // CD changes the folder later relative names start from
    let raw = split_commands_in("CD 'lib'.\nINSERT FILE='more.sps'.", Some(&dir)).unwrap();
    assert_eq!(raw.len(), 3);
    // without FILE=, as the command allows, the file is read and its macros serve later lines
    let raw = split_commands_in("include \"lib/defs.sps\".\n!freq a b.", Some(&dir)).unwrap();
    assert_eq!(raw.last().unwrap().name, "FREQUENCIES");
    let raw = split_commands_in("INSERT 'lib/defs.sps' ERROR=STOP.\n!freq a.", Some(&dir)).unwrap();
    assert_eq!(raw.last().unwrap().name, "FREQUENCIES");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn an_error_after_a_string_over_lines_points_at_a_missing_quote() {
    let (_, errors) = parse("VALUE LABELS q1 1 'one\n2 'two 'x'.");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].hint.as_deref(),
        Some(
            "the string that starts on line 1 runs over several lines; \
             if it was meant to end on line 1, its closing quote is missing"
        )
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

#[test]
fn set_mnest_and_miterate_limit_the_expansion() {
    let source = "DEFINE !a () 1 !ENDDEFINE.\nDEFINE !b () !a !ENDDEFINE.\n\
                  DEFINE !loop () !DO !i = 1 !TO 3 !i !DOEND !ENDDEFINE.\n\
                  SET MNEST=1 MITERATE=2.\nTITLE !b.\nTITLE !loop.\n\
                  SET MNEST=2 MITERATE=3.\nTITLE !b.\nTITLE !loop.";
    let lines = expanded(source);
    assert!(lines[4].contains("nested more than 1 levels"), "{lines:?}");
    assert!(lines[5].contains("more than 2 times"), "{lines:?}");
    assert_eq!(lines[7..], ["8: TITLE 1.", "9: TITLE 1 2 3."]);
}

#[test]
fn the_macro_state_carries_definitions_and_settings_to_the_next_text() {
    let mut state = MacroState::default();
    let library = "DEFINE !pair (!POSITIONAL !TOKENS(1) / !POSITIONAL !TOKENS(1))\n\
                   !1 + !2\n!ENDDEFINE.\nSET MPRINT=YES MNEST=7.";
    split_commands_with(library, None, &mut state, None).unwrap();
    // written out and read back, as a command-line session keeps it
    let state = MacroState::from_syntax(&state.to_syntax());
    assert_eq!(state.names(), ["!PAIR"]);
    let mut later = state.clone();
    let raw = split_commands_with("COMPUTE x = !pair a b.", None, &mut later, None).unwrap();
    assert_eq!(raw[0].echo.as_deref(), Some("COMPUTE x = a + b"));
    assert_eq!(later, state);
    assert!(state.to_syntax().contains("MNEST=7"));
    // with a line range only the definitions on those lines are kept
    let mut ranged = MacroState::default();
    let text = "DEFINE !one () 1 !ENDDEFINE.\nDEFINE !two () 2 !ENDDEFINE.";
    split_commands_with(text, None, &mut ranged, Some((2, 2))).unwrap();
    assert_eq!(ranged.names(), ["!TWO"]);
}

#[test]
fn a_broken_line_outside_the_range_does_not_stop_the_range() {
    let text = "COMPUTE x = 1.\nCOMPUTE y = 557O.\nCOMPUTE z = 2.";
    let mut state = MacroState::default();
    assert!(split_commands_with(text, None, &mut state, None).is_err());
    let raw = split_commands_with(text, None, &mut state, Some((3, 3))).unwrap();
    assert!(raw.iter().any(|c| c.first_line == 3));
    let error = split_commands_with(text, None, &mut state, Some((2, 3))).unwrap_err();
    assert_eq!(error.message, "invalid number 557O");
}
