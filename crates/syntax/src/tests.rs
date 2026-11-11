use super::*;

fn one(source: &str) -> Command {
    let (mut parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    assert_eq!(parsed.len(), 1, "{source}");
    parsed.remove(0).command
}

fn err(source: &str) -> SyntaxError {
    let (_, mut errors) = parse(source);
    assert_eq!(errors.len(), 1, "{source}: {errors:?}");
    errors.remove(0)
}

fn at(error: &SyntaxError) -> (u32, u32) {
    (error.span.line, error.span.column)
}

fn layout(source: &str) -> Vec<(String, u32, u32)> {
    split_commands(source)
        .unwrap()
        .into_iter()
        .map(|c| (c.name, c.first_line, c.last_line))
        .collect()
}

fn rows(expected: &[(&str, u32, u32)]) -> Vec<(String, u32, u32)> {
    expected
        .iter()
        .map(|&(name, first, last)| (name.to_string(), first, last))
        .collect()
}

fn var(name: &str) -> VarItem {
    VarItem::Name(name.to_string())
}

fn range(first: &str, last: &str) -> VarItem {
    VarItem::Range(first.to_string(), last.to_string())
}

fn num(n: f64) -> Value {
    Value::Number(n)
}

fn text(s: &str) -> Value {
    Value::String(s.to_string())
}

/// Fully parenthesised form, so that precedence is visible.
fn show(expr: &Expr) -> String {
    match expr {
        Expr::Number(n) => n.to_string(),
        Expr::String(s) => format!("'{s}'"),
        Expr::Variable(name) => name.clone(),
        Expr::Range(first, last) => format!("{first} TO {last}"),
        Expr::Unary { op, operand } => match op {
            UnaryOp::Neg => format!("(-{})", show(operand)),
            UnaryOp::Not => format!("(NOT {})", show(operand)),
        },
        Expr::Binary { op, left, right } => {
            let symbol = match op {
                BinaryOp::Add => "+",
                BinaryOp::Sub => "-",
                BinaryOp::Mul => "*",
                BinaryOp::Div => "/",
                BinaryOp::Pow => "**",
                BinaryOp::Eq => "=",
                BinaryOp::Ne => "<>",
                BinaryOp::Lt => "<",
                BinaryOp::Le => "<=",
                BinaryOp::Gt => ">",
                BinaryOp::Ge => ">=",
                BinaryOp::And => "AND",
                BinaryOp::Or => "OR",
            };
            format!("({} {symbol} {})", show(left), show(right))
        }
        Expr::Call { name, args } => {
            let args: Vec<String> = args.iter().map(show).collect();
            format!("{name}({})", args.join(", "))
        }
    }
}

fn expr(source: &str) -> String {
    match one(&format!("COMPUTE x = {source}.")) {
        Command::Compute { expr, .. } => show(&expr),
        other => panic!("{other:?}"),
    }
}

// Splitting

#[test]
fn splits_multi_line_commands_with_exact_line_ranges() {
    let source = "GET FILE='a.sav'.\n\
                  VARIABLE LABELS age 'Age'\n  /income 'Income'\n  .\n\
                  \n\
                  FREQUENCIES\n  VARIABLES=age.\n\
                  EXECUTE.";
    let expected = [
        ("GET", 1, 1),
        ("VARIABLE LABELS", 2, 4),
        ("FREQUENCIES", 6, 7),
        ("EXECUTE", 8, 8),
    ];
    assert_eq!(layout(source), rows(&expected));
    assert_eq!(parse(source).0.len(), 4);
}

#[test]
fn blank_line_and_end_of_input_terminate() {
    let source = "COMPUTE a = 1\n\nCOMPUTE b = 2\n  \t \nEXECUTE";
    let expected = [("COMPUTE", 1, 1), ("COMPUTE", 3, 3), ("EXECUTE", 5, 5)];
    assert_eq!(layout(source), rows(&expected));
    let (parsed, errors) = parse(source);
    assert_eq!((parsed.len(), errors.len()), (3, 0));
}

#[test]
fn windows_line_ends_and_byte_order_mark() {
    let source = "\u{feff}GET FILE='a.sav'.\r\nCOMPUTE a = 1\r\n  + 2.\r\n\r\nEXECUTE.\r\n";
    let expected = [("GET", 1, 1), ("COMPUTE", 2, 3), ("EXECUTE", 5, 5)];
    assert_eq!(layout(source), rows(&expected));
}

#[test]
fn comment_commands_and_inline_comments_are_dropped() {
    let source = "* A star comment, isn't it.\n\
                  COMMENT another one\n  that continues here.\n\
                  COMPUTE a = 1 /* inline */ + 2. /* after the period */\n\
                  /* a line of its own */\n\
                  EXECUTE.";
    assert_eq!(
        layout(source),
        rows(&[("COMPUTE", 4, 4), ("EXECUTE", 6, 6)])
    );
    let (parsed, _) = parse(source);
    match &parsed[0].command {
        Command::Compute { expr, .. } => assert_eq!(show(expr), "(1 + 2)"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn comment_without_terminator_swallows_the_next_line() {
    let source = "* no period here\nCOMPUTE a = 1.\nEXECUTE.";
    assert_eq!(layout(source), rows(&[("EXECUTE", 3, 3)]));
    // inside a comment command a trailing inline comment hides the period
    let source = "* text. /* note */\nEXECUTE.\nEXECUTE.";
    assert_eq!(layout(source), rows(&[("EXECUTE", 3, 3)]));
}

#[test]
fn unclosed_inline_comment_hides_the_period() {
    let source = "COMPUTE a = 1 /* note.\n  + 2.\nEXECUTE.";
    assert_eq!(
        layout(source),
        rows(&[("COMPUTE", 1, 2), ("EXECUTE", 3, 3)])
    );
    assert_eq!(parse(source).1, []);
}

#[test]
fn comment_keyword_needs_four_letters() {
    let source = "COMM x = 1.\nCOM y = 2.";
    assert_eq!(layout(source), rows(&[("COMPUTE", 2, 2)]));
}

#[test]
fn empty_commands_are_skipped() {
    assert_eq!(
        layout(".\n\n  .\nEXECUTE.\n/* only a comment */\n"),
        rows(&[("EXECUTE", 4, 4)])
    );
    assert_eq!(layout(""), []);
}

#[test]
fn raw_command_keeps_tokens_and_end_position() {
    let commands = split_commands("COMPUTE a = 1\n  + 2.\nEXECUTE").unwrap();
    assert_eq!(commands[0].tokens.len(), 6);
    assert_eq!(
        commands[0].tokens[5].span,
        Span {
            line: 2,
            column: 5,
            end_line: 2,
            end_column: 6
        }
    );
    assert_eq!((commands[0].end.line, commands[0].end.column), (2, 6));
    // no period: the end is the zero-width position after the last character
    assert_eq!(
        commands[1].end,
        Span {
            line: 3,
            column: 8,
            end_line: 3,
            end_column: 8
        }
    );
}

#[test]
fn split_commands_fails_on_a_lexical_error_only() {
    let error = split_commands("EXECUTE.\nCOMPUTE a = 'open.\n").unwrap_err();
    assert_eq!(error.message, "unterminated string");
    assert_eq!(at(&error), (2, 13));
    // unknown and unsupported commands split fine, even with free text in them
    let expected = [("TITLE", 1, 1), ("BOGUS", 2, 2), ("EXECUTE", 3, 3)];
    assert_eq!(
        layout("TITLE Don't panic.\nBOGUS it's 'open.\nEXECUTE."),
        rows(&expected)
    );
}

// Line selection

const SIX_LINES: &str = "GET FILE='a.sav'.\n\
                         COMPUTE x = 1\n   + 2.\n\
                         \n\
                         FREQUENCIES VARIABLES=x.\n\
                         EXECUTE.";

#[test]
fn select_lines_returns_the_commands_inside_the_range() {
    let commands = split_commands(SIX_LINES).unwrap();
    let names = |first, last| -> Vec<&str> {
        select_lines(&commands, first, last)
            .unwrap()
            .iter()
            .map(|c| c.name.as_str())
            .collect()
    };
    assert_eq!(names(1, 3), ["GET", "COMPUTE"]);
    assert_eq!(names(2, 3), ["COMPUTE"]);
    assert_eq!(names(5, 5), ["FREQUENCIES"]);
    assert_eq!(names(4, 99), ["FREQUENCIES", "EXECUTE"]);
    assert_eq!(names(1, 6).len(), 4);
    assert!(names(4, 4).is_empty());
    assert!(names(7, 9).is_empty());
}

#[test]
fn select_lines_rejects_a_range_that_cuts_a_command() {
    let commands = split_commands(SIX_LINES).unwrap();
    let error = select_lines(&commands, 3, 5).unwrap_err();
    assert_eq!(
        error.message,
        "line range 3-5 cuts through command COMPUTE, which occupies lines 2-3"
    );
    assert_eq!(
        error.hint.as_deref(),
        Some("use lines 2-5 to include the whole command")
    );
    assert_eq!(
        error.span,
        Span {
            line: 2,
            column: 1,
            end_line: 3,
            end_column: 8
        }
    );
    assert!(select_lines(&commands, 1, 2).is_err());
    assert!(select_lines(&commands, 0, 3).is_err());
    assert!(select_lines(&commands, 5, 2).is_err());
}

// Lexical details

#[test]
fn strings_with_doubled_quotes() {
    let source = "VARIABLE LABELS a 'It''s here' b \"Say \"\"hi\"\" now\" \
                  c 'mixed \"inner\"' d \"it's\" e ''.";
    let expected = [
        ("a", "It's here"),
        ("b", "Say \"hi\" now"),
        ("c", "mixed \"inner\""),
        ("d", "it's"),
        ("e", ""),
    ]
    .map(|(name, label)| (name.to_string(), label.to_string()));
    assert_eq!(one(source), Command::VariableLabels(expected.to_vec()));
}

#[test]
fn string_pieces_are_joined_with_plus() {
    let source = "VARIABLE LABELS a 'one ' +\n  'two' + \" three\".";
    let expected = vec![("a".to_string(), "one two three".to_string())];
    assert_eq!(one(source), Command::VariableLabels(expected));
}

#[test]
fn number_forms() {
    assert_eq!(
        expr("1 + 1.5 + .5 + 1. + 1e3 + 2.5E-1 + 1.e1 + -2"),
        "(((((((1 + 1.5) + 0.5) + 1) + 1000) + 0.25) + 10) + (-2))"
    );
    // a number right before the terminator
    assert_eq!(expr("1"), "1");
    assert_eq!(expr("3-2"), "(3 - 2)");
    assert_eq!(expr("3 --2"), "(3 - (-2))");
    let error = err("COMPUTE x = 12abc.");
    assert_eq!(error.message, "invalid number 12abc");
    assert_eq!(at(&error), (1, 13));
}

#[test]
fn names_keep_case_and_allow_special_characters() {
    let source = "COMPUTE Sünni_Aasta.2 = #tmp + @x + $casenum + a.b.";
    match one(source) {
        Command::Compute { target, expr } => {
            assert_eq!(target, "Sünni_Aasta.2");
            assert_eq!(show(&expr), "(((#tmp + @x) + $casenum) + a.b)");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(err("COMPUTE _x = 1.").message, "unexpected character '_'");
}

// Command names

#[test]
fn abbreviated_command_names_and_keywords() {
    assert_eq!(
        one("FREQ VAR=age."),
        Command::Frequencies(Frequencies {
            variables: vec![var("age")],
            ..frequencies_defaults()
        })
    );
    let source = "VAR LAB a 'x'.\nVAL LAB a 1 'x'.\nMIS VAL a (9).\nVARIABLE LEV a (NOM).\n\
                  for a (f2).\nSEL IF a.\nEXE.\nREC a (1=2).\nget fil='x'.\nSav out='y'.\n\
                  VARIABLE\n  LABELS a 'x'.\nselect\nif a.";
    let expected = [
        ("VARIABLE LABELS", 1, 1),
        ("VALUE LABELS", 2, 2),
        ("MISSING VALUES", 3, 3),
        ("VARIABLE LEVEL", 4, 4),
        ("FORMATS", 5, 5),
        ("SELECT IF", 6, 6),
        ("EXECUTE", 7, 7),
        ("RECODE", 8, 8),
        ("GET", 9, 9),
        ("SAVE", 10, 10),
        ("VARIABLE LABELS", 11, 12),
        ("SELECT IF", 13, 14),
    ];
    assert_eq!(layout(source), rows(&expected));
    assert_eq!(parse(source).1, []);
}

#[test]
fn unknown_command_gets_a_did_you_mean_hint() {
    let error = err("FREQUENCES VARIABLES=age.");
    assert_eq!(error.message, "unknown command FREQUENCES");
    assert_eq!(error.hint.as_deref(), Some("did you mean FREQUENCIES?"));
    assert_eq!(at(&error), (1, 1));

    let error = err("EXECUTE.\n  VARIABLE LABLES a 'x'.");
    assert_eq!(error.message, "unknown command VARIABLE");
    assert_eq!(error.hint.as_deref(), Some("did you mean VARIABLE LABELS?"));
    assert_eq!(at(&error), (2, 3));

    let hint = err("VARIABLE a 'x'.").hint.unwrap();
    assert!(
        hint.starts_with("did you mean VARIABLE LABELS, VARIABLE LEVEL, "),
        "{hint}"
    );

    assert_eq!(err("FR x.").hint, None);
    assert_eq!(err("XYZZY 1 2.").hint, None);
    assert_eq!(err("123.").message, "a command must start with its name");
}

#[test]
fn real_but_unsupported_commands_say_so() {
    for (source, name) in [
        ("REGRESSION /DEPENDENT=a.", "REGRESSION"),
        ("factor /variables=a.", "FACTOR"),
        ("GLM a BY b.", "GLM"),
        ("TITLE Don't panic.", "TITLE"),
        ("RELI /VARIABLES=a b.", "RELIABILITY"),
    ] {
        let error = err(source);
        assert_eq!(
            error.message,
            format!("command {name} is not supported yet")
        );
        assert_eq!(at(&error), (1, 1));
        assert!(error.hint.unwrap().contains("FREQUENCIES"));
    }
}

#[test]
fn ambiguous_abbreviation_lists_the_candidates() {
    let error = err("DIS x.");
    assert_eq!(error.message, "ambiguous command abbreviation DIS");
    assert_eq!(error.expected, ["DISPLAY", "DISCRIMINANT"]);
}

// Errors

#[test]
fn error_display_format() {
    assert_eq!(
        err("FREQUENCES VARIABLES=age.").to_string(),
        "line 1, column 1: unknown command FREQUENCES; hint: did you mean FREQUENCIES?"
    );
    assert_eq!(
        err("COMPUTE x = (1 + 2.").to_string(),
        "line 1, column 19: unexpected end of command; expected: ')'"
    );
    assert_eq!(
        err("VARIABLE LEVEL a (BIG).").to_string(),
        "line 1, column 19: unexpected word BIG; expected: SCALE, ORDINAL, NOMINAL"
    );
}

#[test]
fn error_positions_of_typical_mistakes() {
    // an open string runs on over the next lines, so the error points at where it opens
    let source = "GET FILE='a.sav'.\nVARIABLE LABELS age 'Age in years.\nEXECUTE.";
    let (parsed, errors) = parse(source);
    assert_eq!(parsed.len(), 1);
    assert_eq!(errors[0].message, "unterminated string");
    assert_eq!(
        errors[0].span,
        Span {
            line: 2,
            column: 21,
            end_line: 2,
            end_column: 22
        }
    );

    let error = err("EXECUTE. EXECUTE.");
    assert_eq!(error.message, "unexpected character '.'");
    assert_eq!(at(&error), (1, 8));
    assert!(error.hint.unwrap().contains("new line"));

    let error = err("VALUE LABELS gender 1 'M' region 1 'x'.");
    assert_eq!(error.message, "unexpected word region");
    assert_eq!(at(&error), (1, 27));
    assert_eq!(error.expected, ["a value", "'/'", "end of command"]);

    // columns count characters, not bytes
    let error = err("COMPUTE õü = 'ää' ).");
    assert_eq!(error.message, "unexpected ')'");
    assert_eq!(at(&error), (1, 19));

    let error = err("COMPUTE = 5.");
    assert_eq!(error.expected, ["a target variable"]);
    assert_eq!(at(&error), (1, 9));

    let error = err("COMPUTE x = 1\n  + and.");
    assert_eq!(error.message, "unexpected word and");
    assert_eq!(at(&error), (2, 5));

    let error = err("FREQUENCIES VARIABLES=a to.");
    assert_eq!(error.message, "unexpected end of command");
    assert_eq!(error.expected, ["a variable name"]);
    assert_eq!(at(&error), (1, 27));

    // a forgotten period: the next command is read as part of this one
    let error = err("COMPUTE x = 1\nEXECUTE.");
    assert_eq!(error.message, "unexpected word EXECUTE");
    assert_eq!(at(&error), (2, 1));
    assert!(
        error
            .hint
            .unwrap()
            .contains("end the previous command with a period")
    );
    assert_eq!(err("EXECUTE now.").hint, None);

    let error = err("VARIABLE LABELS or 'x'.");
    assert_eq!(
        error.message,
        "OR is a reserved word and cannot be used as a name"
    );
}

#[test]
fn parse_collects_one_error_per_bad_command() {
    let source = "GET FILE=1.\n\
                  COMPUTE x = 1.\n\
                  FORMATS x (F8.2\n\
                  \n\
                  NONSENSE 1 2.\n\
                  EXECUTE.";
    let (parsed, errors) = parse(source);
    let lines: Vec<(u32, u32)> = parsed.iter().map(|p| (p.first_line, p.last_line)).collect();
    assert_eq!(lines, [(2, 2), (6, 6)]);
    assert_eq!(parsed[1].command, Command::Execute);
    let positions: Vec<(u32, u32)> = errors.iter().map(at).collect();
    assert_eq!(positions, [(1, 10), (3, 16), (5, 1)]);
}

// Tier 1 commands

#[test]
fn get_file() {
    let expected = |file: &str| Command::Get {
        file: file.to_string(),
        options: vec![],
    };
    assert_eq!(
        one("GET FILE='data/survey.sav'."),
        expected("data/survey.sav")
    );
    assert_eq!(
        one("get\n  /file = \"C:\\data\\my file.sav\"   ."),
        expected("C:\\data\\my file.sav")
    );

    // a name without quotes is a file handle (FILE HANDLE)
    assert_eq!(one("GET FILE=survey.sav."), expected("survey.sav"));
    let error = err("GET FILE=1.");
    assert_eq!(error.expected, ["a quoted file path or a file handle"]);
    assert_eq!(err("GET.").expected, ["FILE"]);
    let error = err("GET FILE='a.sav' /MAP.");
    assert_eq!(error.message, "subcommand MAP of GET is not supported yet");
    assert_eq!(at(&error), (1, 19));
    assert_eq!(
        err("GET FILE='a.sav' extra.").expected,
        ["'/'", "end of command"]
    );
}

#[test]
fn save_outfile() {
    let expected = |outfile: &str, compression| Command::Save {
        outfile: outfile.to_string(),
        compression,
        options: vec![],
    };
    assert_eq!(one("SAVE OUTFILE='out.sav'."), expected("out.sav", None));
    assert_eq!(
        one("SAVE OUTFILE='out.sav' /COMPRESSED."),
        expected("out.sav", Some(Compression::Compressed))
    );
    assert_eq!(
        one("SAVE OUTFILE='out.sav'\n  /UNCOMPRESSED."),
        expected("out.sav", Some(Compression::Uncompressed))
    );
    assert_eq!(
        one("sav out 'o.zsav'\n /zcomp\n."),
        expected("o.zsav", Some(Compression::ZCompressed))
    );

    assert_eq!(
        err("SAVE OUTFILE='a.sav' /MAP.").message,
        "subcommand MAP of SAVE is not supported yet"
    );
    assert_eq!(
        err("SAVE OUTFILE='a.sav' /FAST.").message,
        "unknown subcommand FAST of SAVE"
    );
    assert_eq!(err("SAVE 'a.sav'.").expected, ["OUTFILE"]);
}

#[test]
fn variable_labels() {
    let pairs = |pairs: &[(&str, &str)]| {
        Command::VariableLabels(
            pairs
                .iter()
                .map(|(name, label)| (name.to_string(), label.to_string()))
                .collect(),
        )
    };
    assert_eq!(
        one("VARIABLE LABELS age 'Age in years'."),
        pairs(&[("age", "Age in years")])
    );
    assert_eq!(
        one("variable labels\n  Q1 \"First\"\n  / q2 'Second' q3 'Third'\n."),
        pairs(&[("Q1", "First"), ("q2", "Second"), ("q3", "Third")])
    );

    let error = err("VARIABLE LABELS age income 'x'.");
    assert_eq!(error.message, "unexpected word income");
    assert_eq!(error.expected, ["a quoted label"]);
    assert_eq!(err("VARIABLE LABELS 'x'.").expected, ["a variable name"]);
}

#[test]
fn value_labels() {
    let group = |vars: Vec<VarItem>, labels: &[(Value, &str)]| ValueLabels {
        vars,
        labels: labels
            .iter()
            .map(|(value, label)| (value.clone(), label.to_string()))
            .collect(),
    };
    assert_eq!(
        one("VALUE LABELS gender 1 'Male' 2 'Female'."),
        Command::ValueLabels(vec![group(
            vec![var("gender")],
            &[(num(1.0), "Male"), (num(2.0), "Female")]
        )])
    );
    let source = "value labels /q1 TO q3, extra -1 'Refused' 1.5 \"Half\"\n\
                  / plan 'b' 'Basic'\n    'p' 'It''s premium'\n\
                  / cleared ALL.";
    assert_eq!(
        one(source),
        Command::ValueLabels(vec![
            group(
                vec![range("q1", "q3"), var("extra")],
                &[(num(-1.0), "Refused"), (num(1.5), "Half")]
            ),
            group(
                vec![var("plan")],
                &[(text("b"), "Basic"), (text("p"), "It's premium")]
            ),
            group(vec![var("cleared"), VarItem::All], &[]),
        ])
    );

    let error = err("VALUE LABELS gender 1 2 'x'.");
    assert_eq!(error.message, "unexpected number 2");
    assert_eq!(error.expected, ["a quoted label"]);
    assert_eq!(err("VALUE LABELS 1 'x'.").expected, ["a variable name"]);
}

#[test]
fn missing_values() {
    use Bound::{Highest, Lowest, Number};
    use MissingValue::{Range, Value as Discrete};
    assert_eq!(
        one("MISSING VALUES q1 q2 (98, 99)."),
        Command::MissingValues(vec![(
            vec![var("q1"), var("q2")],
            vec![Discrete(num(98.0)), Discrete(num(99.0))]
        )])
    );
    let source = "missing values a (LO THRU -1, 999) / b (5 thru HIGHEST) c ('x' 'y')\n\
                  d () e TO f (LOWEST THRU HI) g (1 2 3) h (-9, -5 THR -1).";
    assert_eq!(
        one(source),
        Command::MissingValues(vec![
            (
                vec![var("a")],
                vec![Range(Lowest, Number(-1.0)), Discrete(num(999.0))]
            ),
            (vec![var("b")], vec![Range(Number(5.0), Highest)]),
            (
                vec![var("c")],
                vec![Discrete(text("x")), Discrete(text("y"))]
            ),
            (vec![var("d")], vec![]),
            (vec![range("e", "f")], vec![Range(Lowest, Highest)]),
            (
                vec![var("g")],
                vec![Discrete(num(1.0)), Discrete(num(2.0)), Discrete(num(3.0))]
            ),
            (
                vec![var("h")],
                vec![Discrete(num(-9.0)), Range(Number(-5.0), Number(-1.0))]
            ),
        ])
    );

    let error = err("MISSING VALUES age (1, 2, 3, 4).");
    assert!(error.message.starts_with("too many missing values"));
    assert_eq!(
        error.span,
        Span {
            line: 1,
            column: 21,
            end_line: 1,
            end_column: 31
        }
    );
    assert!(
        err("MISSING VALUES a (1 THRU 2, 5 THRU 6).")
            .message
            .starts_with("too many")
    );
    assert!(
        err("MISSING VALUES a (1 THRU 2, 8, 9).")
            .message
            .starts_with("too many")
    );
    assert_eq!(err("MISSING VALUES a (LO 5).").expected, ["THRU"]);
    assert_eq!(err("MISSING VALUES a 9.").expected, ["'('"]);
    assert_eq!(
        err("MISSING VALUES a (9.").expected,
        ["a value", "a range", "')'"]
    );
}

#[test]
fn variable_level() {
    assert_eq!(
        one("VARIABLE LEVEL age (SCALE)."),
        Command::VariableLevel(vec![(vec![var("age")], Level::Scale)])
    );
    assert_eq!(
        one("variable level a b (sca) / c TO d (ORD)\n  e (Nominal)."),
        Command::VariableLevel(vec![
            (vec![var("a"), var("b")], Level::Scale),
            (vec![range("c", "d")], Level::Ordinal),
            (vec![var("e")], Level::Nominal),
        ])
    );
    assert_eq!(err("VARIABLE LEVEL (SCALE).").expected, ["a variable name"]);
}

#[test]
fn variable_width() {
    assert_eq!(
        one("VARIABLE WIDTH sales95 sales96 (10)\n  /id gender (2)."),
        Command::VariableWidth(vec![
            (vec![var("sales95"), var("sales96")], 10),
            (vec![var("id"), var("gender")], 2),
        ])
    );
    assert_eq!(
        one("var wid a (8) b (3)."),
        Command::VariableWidth(vec![(vec![var("a")], 8), (vec![var("b")], 3)])
    );
    assert_eq!(
        err("VARIABLE WIDTH a (0).").expected,
        ["a column width of at least 1"]
    );
    assert_eq!(
        err("VARIABLE WIDTH a (2.5).").expected,
        ["a column width of at least 1"]
    );
}

#[test]
fn formats() {
    let format = |kind, width, decimals| Format {
        kind,
        width,
        decimals,
    };
    assert_eq!(
        one("FORMATS income (F8.2)."),
        Command::Formats(vec![(vec![var("income")], format(FormatType::F, 8, 2))])
    );
    assert_eq!(
        one("formats a b (a20) / d (DATE11) e TO f ( comma12.2 )\n g (F3)."),
        Command::Formats(vec![
            (vec![var("a"), var("b")], format(FormatType::A, 20, 0)),
            (vec![var("d")], format(FormatType::Date, 11, 0)),
            (vec![range("e", "f")], format(FormatType::Comma, 12, 2)),
            (vec![var("g")], format(FormatType::F, 3, 0)),
        ])
    );
    assert_eq!(FormatType::Datetime.name(), "DATETIME");

    let error = err("FORMATS a (F).");
    assert!(error.message.starts_with("invalid format F"), "{error}");
    assert_eq!(at(&error), (1, 12));
    assert_eq!(
        err("FORMATS a (XYZ8.2).").message,
        "unknown format type in XYZ8.2"
    );
    assert_eq!(err("FORMATS a (8.2).").expected, ["a format such as F8.2"]);
}

pub(crate) fn frequencies_defaults() -> Frequencies {
    Frequencies {
        variables: Vec::new(),
        notable: false,
        limit: None,
        order: FrequencyOrder::AValue,
        include: false,
        statistics: Vec::new(),
        percentiles: Vec::new(),
        grouped: Vec::new(),
        charts: Vec::new(),
        by_variable: false,
    }
}

#[test]
fn frequencies() {
    let expected = |variables| {
        Command::Frequencies(Frequencies {
            variables,
            ..frequencies_defaults()
        })
    };
    assert_eq!(
        one("FREQUENCIES VARIABLES=gender region."),
        expected(vec![var("gender"), var("region")])
    );
    assert_eq!(
        one("freq /var = a, b TO c\n  ALL."),
        expected(vec![var("a"), range("b", "c"), VarItem::All])
    );
    // the bare form, with a variable whose name looks like the keyword
    assert_eq!(
        one("FREQUENCIES gender var."),
        expected(vec![var("gender"), var("var")])
    );
    assert_eq!(
        err("FREQUENCIES VARIABLES=age /FOO.").message,
        "unknown subcommand FOO of FREQUENCIES"
    );
    assert_eq!(err("FREQUENCIES.").expected, ["a variable name"]);
    let error = err("FREQUENCIES VARIABLES age.");
    assert_eq!(error.expected, ["'='"]);
    assert_eq!(at(&error), (1, 23));
}

#[test]
fn execute() {
    assert_eq!(one("EXECUTE."), Command::Execute);
    assert_eq!(one("  exe\n ."), Command::Execute);
    let error = err("EXECUTE now.");
    assert_eq!(error.message, "unexpected word now");
    assert_eq!(error.expected, ["end of command"]);
}

// Expressions

#[test]
fn operator_precedence() {
    for (source, expected) in [
        ("1 + 2 * 3", "(1 + (2 * 3))"),
        ("1 - 2 - 3", "((1 - 2) - 3)"),
        ("8 / 4 / 2", "((8 / 4) / 2)"),
        ("(1 + 2) * 3", "((1 + 2) * 3)"),
        ("-2 ** 2", "(-(2 ** 2))"),
        ("2 ** 3 ** 2", "((2 ** 3) ** 2)"),
        ("2 ** -1", "(2 ** (-1))"),
        ("-a * b", "((-a) * b)"),
        ("a * -b", "(a * (-b))"),
        ("a - -b", "(a - (-b))"),
        ("+a", "a"),
        ("a + b > c * d", "((a + b) > (c * d))"),
        ("NOT a = b AND c OR d", "(((NOT (a = b)) AND c) OR d)"),
        ("a OR b AND c", "(a OR (b AND c))"),
        ("a AND NOT b OR c", "((a AND (NOT b)) OR c)"),
        ("NOT NOT a", "(NOT (NOT a))"),
        ("a | b & ~c", "(a OR (b AND (NOT c)))"),
        (
            "a = 1 OR b = 2 AND c = 3",
            "((a = 1) OR ((b = 2) AND (c = 3)))",
        ),
    ] {
        assert_eq!(expr(source), expected, "{source}");
    }
}

#[test]
fn comparison_and_logical_operator_spellings() {
    assert_eq!(
        expr("a eq 1 or b lt 2 and c gt 3"),
        "((a = 1) OR ((b < 2) AND (c > 3)))"
    );
    assert_eq!(
        expr("a <= 1 & b >= 2 | c ~= 3"),
        "(((a <= 1) AND (b >= 2)) OR (c <> 3))"
    );
    assert_eq!(
        expr("a LE 1 AND b GE 2 AND c NE 3 AND d <> 4 AND e < 5 AND f > 6"),
        "((((((a <= 1) AND (b >= 2)) AND (c <> 3)) AND (d <> 4)) AND (e < 5)) AND (f > 6))"
    );
}

#[test]
fn function_calls() {
    assert_eq!(
        expr("MEAN.2(a, b, c) + SUM(q1 TO q5) / trunc (x)"),
        "(MEAN.2(a, b, c) + (SUM(q1 TO q5) / trunc(x)))"
    );
    assert_eq!(
        expr("MAX(a, MIN(b, 2) * 3, -1)"),
        "MAX(a, (MIN(b, 2) * 3), (-1))"
    );
    assert_eq!(
        expr("CONCAT('it''s ', name) = \"x\""),
        "(CONCAT('it's ', name) = 'x')"
    );
    assert_eq!(
        expr("ANY(q1, 1, 2) AND NOT MISSING(q2)"),
        "(ANY(q1, 1, 2) AND (NOT MISSING(q2)))"
    );
    assert_eq!(expr("NOW()"), "NOW()");
    assert_eq!(expr("SUM(a TO b, c, d TO e)"), "SUM(a TO b, c, d TO e)");
}

#[test]
fn expression_errors() {
    let error = err("COMPUTE x = 1 + .");
    assert_eq!(error.message, "unexpected end of command");
    assert_eq!(
        error.expected,
        ["a value", "a variable", "a function", "'('"]
    );

    let error = err("COMPUTE x = a b.");
    assert_eq!(error.message, "unexpected word b");
    assert_eq!(error.expected, ["an operator", "end of command"]);
    assert_eq!(at(&error), (1, 15));

    let error = err("COMPUTE x = SUM(a, b.");
    assert_eq!(error.expected, ["','", "')'", "an operator"]);

    let deep = |open: &str, count: usize| format!("COMPUTE x = {}1.", open.repeat(count));
    assert!(
        parse(&deep("(", 50)).1[0]
            .message
            .starts_with("unexpected end")
    );
    for open in ["(", "-", "NOT ", "F("] {
        let error = err(&deep(open, 100_000));
        assert!(error.message.starts_with("expression is nested too deeply"));
    }
    // long operator chains are fine up to a limit
    assert_eq!(parse(&deep("1 + ", 5000)).1, []);
    for open in ["1 + ", "2 ** -", "a AND "] {
        let error = err(&deep(open, 100_000));
        assert!(
            error.message.starts_with("expression is too long"),
            "{error}"
        );
    }

    assert_eq!(err("COMPUTE x = 2 * * 3.").message, "unexpected '*'");
    assert_eq!(err("COMPUTE x = a == 1.").message, "unexpected '='");
    assert!(err("COMPUTE x = a != 1.").hint.unwrap().contains("<>"));
}

// Tier 3 commands

#[test]
fn compute() {
    match one("compute Total = price * qty\n  + shipping.") {
        Command::Compute { target, expr } => {
            assert_eq!(target, "Total");
            assert_eq!(show(&expr), "((price * qty) + shipping)");
        }
        other => panic!("{other:?}"),
    }
    let error = err("COMPUTE v(1 = 2.");
    assert_eq!(at(&error), (1, 16));
    assert_eq!(err("COMPUTE x 5.").expected, ["'='"]);
}

#[test]
fn if_command() {
    let parts = |source: &str| match one(source) {
        Command::If {
            condition,
            target,
            expr,
        } => (show(&condition), target, show(&expr)),
        other => panic!("{other:?}"),
    };
    assert_eq!(
        parts("IF (age >= 65) senior = 1."),
        (
            "(age >= 65)".to_string(),
            "senior".to_string(),
            "1".to_string()
        )
    );
    assert_eq!(
        parts("if age > 30 & g = 1\n  flag = flag + 2."),
        (
            "((age > 30) AND (g = 1))".to_string(),
            "flag".to_string(),
            "(flag + 2)".to_string()
        )
    );
    assert_eq!(
        parts("IF (a = 1) OR (b = 2) c = 'yes'."),
        (
            "((a = 1) OR (b = 2))".to_string(),
            "c".to_string(),
            "'yes'".to_string()
        )
    );
    assert_eq!(err("IF (a > 1).").expected, ["a target variable"]);
    assert_eq!(err("IF (a > 1) b.").expected, ["'='"]);
}

#[test]
fn select_if() {
    match one("SELECT IF (age >= 18 AND NOT MISSING(q1)).") {
        Command::SelectIf(condition) => {
            assert_eq!(show(&condition), "((age >= 18) AND (NOT MISSING(q1)))")
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(
        one("sel if a."),
        Command::SelectIf(Expr::Variable("a".to_string()))
    );
    assert_eq!(
        err("SELECT IF a = 1 b.").expected,
        ["an operator", "end of command"]
    );
    assert_eq!(err("SELECT a = 1.").message, "unknown command SELECT");
}

#[test]
fn recode() {
    use Bound::{Highest, Lowest, Number};
    use RecodeInput as In;
    use RecodeOutput as Out;
    let map = |inputs: Vec<RecodeInput>, output| RecodeMapping { inputs, output };
    let source =
        "RECODE age (LO THRU 29=1) (30 THRU 49=2) (50 THRU HI=3) (ELSE=SYSMIS) INTO agegrp.";
    assert_eq!(
        one(source),
        Command::Recode(vec![Recode {
            vars: vec![var("age")],
            mappings: vec![
                map(vec![In::Range(Lowest, Number(29.0))], Out::Value(num(1.0))),
                map(
                    vec![In::Range(Number(30.0), Number(49.0))],
                    Out::Value(num(2.0))
                ),
                map(vec![In::Range(Number(50.0), Highest)], Out::Value(num(3.0))),
                map(vec![In::Else], Out::Sysmis),
            ],
            into: vec![var("agegrp")],
        }])
    );
    let source = "recode g1 g2 (1,2=1)(3=copy)(MISSING=9)\n\
                  / region TO zone (1 2 3 = 0) (SYSMIS=-1) (-5 thru -1, 99 = sysmis) (ELSE=COPY)\n\
                  / plan ('b'='Basic') (ELSE='Other') INTO plan2 plan3.";
    assert_eq!(
        one(source),
        Command::Recode(vec![
            Recode {
                vars: vec![var("g1"), var("g2")],
                mappings: vec![
                    map(
                        vec![In::Value(num(1.0)), In::Value(num(2.0))],
                        Out::Value(num(1.0))
                    ),
                    map(vec![In::Value(num(3.0))], Out::Copy),
                    map(vec![In::Missing], Out::Value(num(9.0))),
                ],
                into: vec![],
            },
            Recode {
                vars: vec![range("region", "zone")],
                mappings: vec![
                    map(
                        vec![
                            In::Value(num(1.0)),
                            In::Value(num(2.0)),
                            In::Value(num(3.0))
                        ],
                        Out::Value(num(0.0))
                    ),
                    map(vec![In::Sysmis], Out::Value(num(-1.0))),
                    map(
                        vec![In::Range(Number(-5.0), Number(-1.0)), In::Value(num(99.0))],
                        Out::Sysmis
                    ),
                    map(vec![In::Else], Out::Copy),
                ],
                into: vec![],
            },
            Recode {
                vars: vec![var("plan")],
                mappings: vec![
                    map(vec![In::Value(text("b"))], Out::Value(text("Basic"))),
                    map(vec![In::Else], Out::Value(text("Other"))),
                ],
                into: vec![var("plan2"), var("plan3")],
            },
        ])
    );

    assert_eq!(
        err("RECODE a (1=).").expected,
        ["a value", "SYSMIS", "COPY"]
    );
    assert_eq!(err("RECODE a (1 2).").expected, ["a value", "'='"]);
    assert_eq!(err("RECODE a (=2).").message, "unexpected '='");
    assert_eq!(err("RECODE a.").expected, ["'('"]);
    assert_eq!(err("RECODE a (1=2) INTO.").expected, ["a variable name"]);
    let error = err("RECODE a (CONVERT) INTO b.");
    assert_eq!(error.message, "CONVERT in RECODE is not supported yet");
    assert_eq!(at(&error), (1, 11));
}

// Help

#[test]
fn help_for_one_command() {
    let help_text = help(Some("VALUE LABELS")).unwrap();
    assert!(help_text.starts_with("VALUE LABELS: Replace the value labels of variables."));
    assert!(help_text.contains("VALUE LABELS varlist value 'label'"));
    assert!(help_text.contains("Example:\n  VALUE LABELS gender 1 'Male' 2 'Female'"));
    assert_eq!(help(Some("val lab")), Some(help_text));
    assert!(
        help(Some("regression"))
            .unwrap()
            .contains("not supported yet")
    );
    assert!(help(Some("t-test")).unwrap().starts_with("T-TEST: "));
    assert_eq!(help(Some("NOPE")), None);
    assert_eq!(help(Some("VALUE LABELS extra")), None);
    assert_eq!(help(Some("")), None);
}

#[test]
fn command_names_resolve_from_their_words() {
    assert_eq!(command_name(&["fre", "gender"]), Some(("FREQUENCIES", 1)));
    assert_eq!(
        command_name(&["VAL", "LAB", "x"]),
        Some(("VALUE LABELS", 2))
    );
    assert_eq!(command_name(&["T-TEST"]), Some(("T-TEST", 1)));
    // recognised but not supported yet
    assert_eq!(command_name(&["SET", "x"]), Some(("SET", 1)));
    assert_eq!(command_name(&["gender"]), None);
    assert_eq!(command_name(&[]), None);
}

#[test]
fn help_lists_every_command() {
    let overview = help(None).unwrap();
    assert_eq!(command_names().len(), 95);
    for spec in table::COMMANDS {
        assert!(overview.contains(spec.name) && overview.contains(spec.summary));
    }
    assert!(overview.contains("  SELECT IF         Keep only the cases"));
    assert!(overview.contains("  RENAME VARIABLES  Change the names"));
    assert!(overview.contains("General rules:"));
}

#[test]
fn table_entries_cannot_drift_from_the_parser() {
    for spec in table::COMMANDS {
        // an example of a block command (LOOP, DO REPEAT) holds the whole block
        let raw = split_commands(spec.example).unwrap();
        assert!(raw.iter().any(|r| r.name == spec.name), "{}", spec.name);
        let parsed = parse_raws(&raw);
        assert!(parsed.iter().all(|(_, r)| r.is_ok()), "{}", spec.example);
        assert!(spec.syntax.starts_with(spec.name), "{}", spec.name);
        assert!(
            help(Some(spec.name))
                .unwrap()
                .contains(spec.syntax.lines().next().unwrap())
        );
    }
}

// A realistic file

const SURVEY_SCRIPT: &str = "\
* Survey cleaning script, wave 3.
* Loads the raw file and prepares the analysis variables.

GET FILE='data/wave3_raw.sav'.

/* Dictionary */
VARIABLE LABELS
  resp_id 'Respondent number'
  /age 'Age in years'
  /q1 \"Overall satisfaction (don't know = 98)\".
VALUE LABELS q1 TO q3
    1 'Very dissatisfied'
    5 'Very satisfied'
    98 'Don''t know'
  / gender 1 'Male' 2 'Female'.
MISSING VALUES q1 TO q3 (98, 99) / income (LOWEST THRU -1).
VARIABLE LEVEL q1 TO q3 (ORDINAL) / age income (SCALE).
FORMATS income (COMMA10.2) / age (F3).

COMMENT Derived variables
  follow below.
COMPUTE satisfaction = MEAN.2(q1, q2, q3).
IF (age >= 65 AND gender = 2) senior_woman = 1.
RECODE age
  (LOWEST THRU 29 = 1)
  (30 THRU 59 = 2)   /* working age */
  (60 THRU HIGHEST = 3)
  (ELSE = SYSMIS)
  INTO age_group.
SELECT IF NOT MISSING(satisfaction)

FREQ VAR=age_group gender.
SORT CASES BY resp_id.
SAVE OUTFILE='data/wave3_clean.sav'
  /COMPRESSED.
EXECUTE.
";

#[test]
fn realistic_script_names_and_line_ranges() {
    let expected = [
        ("GET", 4, 4),
        ("VARIABLE LABELS", 7, 10),
        ("VALUE LABELS", 11, 15),
        ("MISSING VALUES", 16, 16),
        ("VARIABLE LEVEL", 17, 17),
        ("FORMATS", 18, 18),
        ("COMPUTE", 22, 22),
        ("IF", 23, 23),
        ("RECODE", 24, 29),
        ("SELECT IF", 30, 30),
        ("FREQUENCIES", 32, 32),
        ("SORT CASES", 33, 33),
        ("SAVE", 34, 35),
        ("EXECUTE", 36, 36),
    ];
    assert_eq!(layout(SURVEY_SCRIPT), rows(&expected));

    let (parsed, errors) = parse(SURVEY_SCRIPT);
    assert_eq!(parsed.len(), 14);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!((parsed[8].first_line, parsed[8].last_line), (24, 29));

    let commands = split_commands(SURVEY_SCRIPT).unwrap();
    let block = select_lines(&commands, 19, 31).unwrap();
    let names: Vec<&str> = block.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["COMPUTE", "IF", "RECODE", "SELECT IF"]);
    let error = select_lines(&commands, 22, 26).unwrap_err();
    assert!(
        error
            .message
            .ends_with("command RECODE, which occupies lines 24-29")
    );
}

// Declarations and DO IF

#[test]
fn string_and_numeric() {
    let a = |width| Format {
        kind: FormatType::A,
        width,
        decimals: 0,
    };
    let names = |list: &[&str]| -> Vec<String> { list.iter().map(|n| n.to_string()).collect() };
    assert_eq!(
        one("STRING code1, code2 (A8) / memo (a40)."),
        Command::String(vec![
            (names(&["code1", "code2"]), a(8)),
            (names(&["memo"]), a(40))
        ])
    );
    let f51 = Format {
        kind: FormatType::F,
        width: 5,
        decimals: 1,
    };
    assert_eq!(
        one("NUMERIC score (F5.1) / flag other."),
        Command::Numeric(vec![
            (names(&["score"]), Some(f51)),
            (names(&["flag", "other"]), None)
        ])
    );
    assert_eq!(err("STRING memo.").expected, ["'('"]);
    assert_eq!(
        err("STRING memo (F8.2).").message,
        "STRING needs an A format such as A20"
    );
    assert_eq!(
        err("NUMERIC score (A8).").message,
        "NUMERIC needs a numeric format such as F8.2"
    );
    assert_eq!(err("NUMERIC score (F5.1) extra").expected[0], "'/'");
    assert!(err("STRING a TO c (A8).").message.contains("reserved word"));
}

#[test]
fn do_if_block_commands() {
    let source = "DO IF (age < 18).\nELSE IF age < 65.\nELSE.\nEND IF.\ndo if a.\nend if.";
    let names: Vec<String> = layout(source).into_iter().map(|row| row.0).collect();
    assert_eq!(
        names,
        ["DO IF", "ELSE IF", "ELSE", "END IF", "DO IF", "END IF"]
    );
    let (parsed, errors) = parse(source);
    assert_eq!(errors, []);
    match (&parsed[0].command, &parsed[1].command) {
        (Command::DoIf(first), Command::ElseIf(second)) => {
            assert_eq!(show(first), "(age < 18)");
            assert_eq!(show(second), "(age < 65)");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(parsed[2].command, Command::Else);
    assert_eq!(parsed[3].command, Command::EndIf);
    assert_eq!(err("DO IF.").expected[0], "a value");
    assert_eq!(err("ELSE age < 3.").expected, ["end of command"]);
    assert_eq!(err("END IF age.").expected, ["end of command"]);
}
