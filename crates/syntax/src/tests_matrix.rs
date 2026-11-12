//! MATRIX ... END MATRIX: statements, blocks, expressions and their errors.

use super::*;

fn program(body: &str) -> Vec<MatrixStatement> {
    let source = format!("MATRIX.\n{body}\nEND MATRIX.\n");
    let (mut parsed, errors) = parse(&source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    assert_eq!(parsed.len(), 1, "{source}");
    match parsed.remove(0).command {
        Command::Matrix(program) => program.statements,
        other => panic!("not a matrix program: {other:?}"),
    }
}

fn errors(body: &str) -> Vec<SyntaxError> {
    let source = format!("MATRIX.\n{body}\nEND MATRIX.\n");
    let (_, errors) = parse(&source);
    assert!(!errors.is_empty(), "{source} parsed");
    errors
}

fn one_error(body: &str) -> (String, u32, u32) {
    let errors = errors(body);
    assert_eq!(errors.len(), 1, "{errors:?}");
    let e = &errors[0];
    (e.message.clone(), e.span.line, e.span.column)
}

fn expr_of(statement: &MatrixStatement) -> &MExpr {
    match &statement.kind {
        StatementKind::Compute { expr, .. } => expr,
        other => panic!("not COMPUTE: {other:?}"),
    }
}

/// An expression as a compact tree: operators and functions with their operands.
fn shape(e: &MExpr) -> String {
    match &e.kind {
        ExprKind::Number(n) => n.to_string(),
        ExprKind::String(s) => format!("'{s}'"),
        ExprKind::Variable(v) => v.clone(),
        ExprKind::Matrix(rows) => {
            let rows: Vec<String> = rows
                .iter()
                .map(|r| r.iter().map(shape).collect::<Vec<_>>().join(","))
                .collect();
            format!("{{{}}}", rows.join(";"))
        }
        ExprKind::Index { name, indexes } => {
            let parts: Vec<String> = indexes
                .iter()
                .map(|i| match i {
                    Index::All => ":".into(),
                    Index::Expr(e) => shape(e),
                })
                .collect();
            format!("{name}[{}]", parts.join(","))
        }
        ExprKind::Call { name, args } => {
            format!(
                "{name}({})",
                args.iter().map(shape).collect::<Vec<_>>().join(",")
            )
        }
        ExprKind::Eof(file) => format!("EOF({file})"),
        ExprKind::Neg(x) => format!("(-{})", shape(x)),
        ExprKind::Not(x) => format!("(NOT {})", shape(x)),
        ExprKind::Binary { op, left, right } => {
            format!("({} {op:?} {})", shape(left), shape(right))
        }
        ExprKind::Sequence { first, last, step } => match step {
            Some(step) => format!("({}:{}:{})", shape(first), shape(last), shape(step)),
            None => format!("({}:{})", shape(first), shape(last)),
        },
    }
}

fn compute(text: &str) -> String {
    shape(expr_of(&program(&format!("COMPUTE X = {text}."))[0]))
}

#[test]
fn operators_bind_as_the_manual_lists_them() {
    // unary minus binds tightest, then the colon, then the powers
    assert_eq!(compute("-2 ** 2"), "((-2) Pow 2)");
    assert_eq!(compute("1:3 + 1"), "((1:3) Add 1)");
    assert_eq!(
        compute("A &* B + C &/ D"),
        "((A ElemMul B) Add (C ElemDiv D))"
    );
    assert_eq!(compute("2 &** A ** 2"), "((2 ElemPow A) Pow 2)");
    assert_eq!(compute("A * B / C"), "((A Mul B) Div C)");
    assert_eq!(compute("NOT A > B AND C"), "((NOT (A Gt B)) And C)");
    assert_eq!(compute("A OR B AND C XOR D"), "((A Or (B And C)) Xor D)");
    assert_eq!(compute("A = B"), "(A Eq B)");
    assert_eq!(compute("A GE 1 & ~B | C"), "(((A Ge 1) And (NOT B)) Or C)");
    assert_eq!(compute("A ~= B"), "(A Ne B)");
    assert_eq!(compute("(C &/ 2) < B"), "((C ElemDiv 2) Lt B)");
}

#[test]
fn matrices_ranges_indexes_and_calls() {
    assert_eq!(compute("{1, 2, 3; 4, 5, 6}"), "{1,2,3;4,5,6}");
    assert_eq!(compute("{1:7:2; 2:11:3}"), "{(1:7:2);(2:11:3)}");
    assert_eq!(compute("{}"), "{}");
    assert_eq!(compute("{M; V}"), "{M;V}");
    assert_eq!(compute("M(1, :)"), "M[1,:]");
    assert_eq!(compute("M(:, J)"), "M[:,J]");
    assert_eq!(compute("M(1:3, 4)"), "M[(1:3),4]");
    assert_eq!(compute("V(2)"), "V[2]");
    assert_eq!(compute("inv(A) * t(B)"), "(INV(A) Mul TRANSPOS(B))");
    assert_eq!(compute("ALL(A)"), "ALL(A)");
    assert_eq!(compute("CDF.NORMAL(X, 0, 1)"), "CDF.NORMAL(X,0,1)");
    assert_eq!(compute("BLOCK(A, B, C)"), "BLOCK(A,B,C)");
    assert_eq!(compute("EOF('data.txt')"), "EOF(data.txt)");
    assert_eq!(compute("{'Observe', \"Predict\"}"), "{'Observe','Predict'}");
}

#[test]
fn blocks_nest() {
    let statements = program(
        "LOOP J = 1 TO NROW(E).\n\
         + DO IF (E(J) > 1.0).\n\
         +   PRINT E(J) / TITLE=\"Eigenvalue:\" /SPACE=3.\n\
         + ELSE IF E(J) = 0.\n\
         +   BREAK.\n\
         + ELSE.\n\
         +   COMPUTE X = 1.\n\
         + END IF.\n\
         END LOOP IF X > 2.",
    );
    assert_eq!(statements.len(), 1);
    let StatementKind::Loop(spec) = &statements[0].kind else {
        panic!("{statements:?}");
    };
    let index = spec.index.as_ref().unwrap();
    assert_eq!(index.name, "J");
    assert_eq!(shape(&index.last), "NROW(E)");
    assert!(spec.until.is_some() && spec.condition.is_none());
    let StatementKind::DoIf {
        branches,
        otherwise,
    } = &spec.body[0].kind
    else {
        panic!("{spec:?}");
    };
    assert_eq!(branches.len(), 2);
    assert!(matches!(branches[1].1[0].kind, StatementKind::Break));
    assert_eq!(otherwise.as_ref().unwrap().len(), 1);
    // the manual's example writes a comma for TO
    let statements = program("LOOP L = 1, NROW(V).\nEND LOOP.");
    assert!(matches!(&statements[0].kind, StatementKind::Loop(l) if l.index.is_some()));
    let statements = program("LOOP IF K < 5.\nCOMPUTE K = K + 1.\nEND LOOP.");
    assert!(
        matches!(&statements[0].kind, StatementKind::Loop(l) if l.index.is_none() && l.condition.is_some())
    );
}

#[test]
fn statements_take_abbreviations_and_their_subcommands() {
    let statements = program(
        "COMP X = 1.\nPRIN X /FORMAT=F8.2 /TITLE='x' /RLABELS=a, 'b' /CNAMES=N.\nDISP DIC.\n\
         REL X, Y.\nCALL EIGEN(A, V, E).\nCALL SETDIAG(M, 0).\nCALL SVD(A, U, Q, W).",
    );
    assert_eq!(statements.len(), 7);
    let StatementKind::Print(print) = &statements[1].kind else {
        panic!();
    };
    assert_eq!(print.format.as_deref(), Some("F8.2"));
    assert_eq!(print.title.as_deref(), Some("x"));
    // unquoted labels are upper case
    assert_eq!(
        print.rlabels,
        Some(Labels::List(vec!["A".into(), "b".into()]))
    );
    assert!(matches!(print.clabels, Some(Labels::Names(_))));
    assert!(matches!(
        statements[2].kind,
        StatementKind::Display {
            dictionary: true,
            status: false
        }
    ));
    assert_eq!(
        statements[3].kind,
        StatementKind::Release(vec!["X".into(), "Y".into()])
    );
    // the expression title is the expression as written
    let statements = program("PRINT {1, 2; 3, 4} ** 2.\nPRINT /TITLE='only a title'.");
    let StatementKind::Print(print) = &statements[0].kind else {
        panic!();
    };
    assert_eq!(print.text, "{1, 2; 3, 4} ** 2");
    assert!(matches!(&statements[1].kind, StatementKind::Print(p) if p.expr.is_none()));
}

#[test]
fn files_carry_over_to_later_statements() {
    let statements = program(
        "READ A /FILE='m.txt' /FIELD=1 TO 60 /SIZE={6, 6}.\n\
         READ B(:, :) /FIELD=1 TO 9 BY 3 /SIZE={3, 3} /MODE=SYMMETRIC /REREAD.\n\
         WRITE A /OUTFILE=out /FIELD=1 TO 80 BY 10.\nWRITE (A + 1) /FIELD=1 TO 80 /FORMAT='8F'.\n\
         SAVE A /OUTFILE=* /VARIABLES=x1 TO x6.\nSAVE INV(A).\n\
         MSAVE M /TYPE=MEAN /OUTFILE='c.sav' /VARIABLES=V1 TO V3 /FNAMES=SEX /FACTOR=1.\n\
         MSAVE S /TYPE STDDEV.",
    );
    let StatementKind::Read(second) = &statements[1].kind else {
        panic!();
    };
    assert_eq!(second.file, "m.txt");
    assert!(second.symmetric && second.reread);
    assert_eq!(shape(second.size.as_ref().unwrap()), "{3,3}");
    let StatementKind::Write(second) = &statements[3].kind else {
        panic!();
    };
    assert_eq!(second.outfile, "out");
    assert_eq!(second.format.unwrap().repeat, Some(8));
    let StatementKind::Save(first) = &statements[4].kind else {
        panic!();
    };
    assert_eq!(first.variables.as_ref().unwrap().len(), 6);
    let StatementKind::Save(second) = &statements[5].kind else {
        panic!();
    };
    assert_eq!(second.outfile, None);
    let StatementKind::Msave(second) = &statements[7].kind else {
        panic!();
    };
    assert_eq!(second.outfile, "c.sav");
    assert_eq!(second.variables.as_ref().unwrap(), &["V1", "V2", "V3"]);
    assert_eq!(second.fnames.as_ref().unwrap(), &["SEX"]);
    // FACTOR does not carry over
    assert!(second.factor.is_none());
}

#[test]
fn get_and_mget() {
    let statements = program(
        "GET SCORES /VARIABLES = TEST1,TEST2,TEST3 /NAMES = VARNAMES /MISSING = ACCEPT /SYSMIS = -1.0.\n\
         GET M /FILE=*.\nMGET /FILE='c.sav' /TYPE=CORR MEAN.\nMGET FILE=c.",
    );
    let StatementKind::Get(get) = &statements[0].kind else {
        panic!();
    };
    assert_eq!(get.variables.as_ref().unwrap().len(), 3);
    assert_eq!(get.missing, Some(MissingAction::Accept));
    assert_eq!(get.sysmis, Some(MissingAction::Value(-1.0)));
    let StatementKind::Mget(mget) = &statements[2].kind else {
        panic!();
    };
    assert_eq!(mget.types, ["CORR", "MEAN"]);
    assert!(
        matches!(&statements[3].kind, StatementKind::Mget(m) if m.file.as_deref() == Some("c"))
    );
}

#[test]
fn errors_point_at_their_statement() {
    assert_eq!(
        one_error("COMPUTE A = 1.\nFREQUENCIES A."),
        (
            "FREQUENCIES is not a statement of a matrix program".into(),
            3,
            1
        )
    );
    assert_eq!(one_error("BREAK."), ("BREAK outside a loop".into(), 2, 1));
    assert_eq!(
        one_error("DO IF 1.\nCOMPUTE A = 1."),
        ("DO IF without END IF".into(), 2, 1)
    );
    assert_eq!(
        one_error("LOOP.\nEND IF."),
        ("END IF, ELSE IF or ELSE inside a LOOP".into(), 3, 1)
    );
    assert_eq!(
        one_error("END LOOP."),
        ("END LOOP without LOOP".into(), 2, 1)
    );
    assert_eq!(
        one_error("DO IF 1.\nELSE.\nELSE.\nEND IF."),
        ("ELSE IF or ELSE after ELSE".into(), 4, 1)
    );
    let (message, line, column) = one_error("PRINT A + B.");
    assert_eq!((message.as_str(), line, column), ("unexpected '+'", 2, 9));
    assert_eq!(
        errors("PRINT A + B.")[0]
            .hint
            .as_deref()
            .map(|h| h.contains("PRINT (A+B)")),
        Some(true)
    );
    assert_eq!(
        one_error("COMPUTE A = INV(B, C)."),
        ("INV takes 1 argument, got 2".into(), 2, 13)
    );
    assert_eq!(
        one_error("COMPUTE T = 1."),
        (
            "T is the name of a matrix function or procedure and cannot name a matrix".into(),
            2,
            9
        )
    );
    assert_eq!(
        one_error("COMPUTE A = DET."),
        (
            "DET is a matrix function and needs its arguments in parentheses".into(),
            2,
            13
        )
    );
    assert_eq!(
        one_error("READ A /FIELD=1 TO 10 /SIZE=2."),
        ("the first READ of the program needs FILE".into(), 2, 6)
    );
    assert_eq!(
        one_error("READ A /FILE='x' /FIELD=1 TO 10."),
        ("READ of a whole matrix needs SIZE".into(), 2, 6)
    );
    assert_eq!(
        one_error("READ A /FILE='x' /FIELD=1 TO 10 BY 3 /SIZE=2."),
        (
            "the width 3 does not divide the 10 columns of the field evenly".into(),
            2,
            36
        )
    );
    assert_eq!(
        one_error("RELEASE A B."),
        ("unexpected word B".into(), 2, 11)
    );
    assert_eq!(
        one_error("MSAVE A /OUTFILE='x'."),
        ("MSAVE needs TYPE".into(), 2, 7)
    );
    assert_eq!(
        one_error("MSAVE A /TYPE=MEAN /OUTFILE='x'.\nMSAVE B /TYPE=N /OUTFILE='y'."),
        (
            "a matrix program writes one matrix file; an earlier MSAVE writes x".into(),
            3,
            26
        )
    );
    assert_eq!(
        one_error("SAVE A."),
        ("the first SAVE of the program needs OUTFILE".into(), 2, 6)
    );
    assert_eq!(
        one_error("CALL FOO(A)."),
        ("unknown matrix procedure FOO".into(), 2, 6)
    );
    assert_eq!(
        one_error("COMPUTE A = M(1, 2, 3)."),
        ("unexpected ','".into(), 2, 19)
    );
    let (_, errors) = parse("MATRIX.\nCOMPUTE A = 1.\n");
    assert_eq!(errors[0].message, "MATRIX without END MATRIX");
    let (_, errors) = parse("MATRIX.\nEND MATRIX extra.\n");
    assert_eq!(errors[0].message, "END MATRIX takes nothing more");
    // every bad statement gives its own error
    assert_eq!(
        errors_count("COMPUTE = 1.\nPRINT A + B.\nCOMPUTE B = 2."),
        2
    );
}

fn errors_count(body: &str) -> usize {
    errors(body).len()
}

#[test]
fn the_matrix_tokens_belong_to_the_program() {
    // outside MATRIX braces are no tokens
    let (_, errors) = parse("COMPUTE x = {1}.\n");
    assert_eq!(errors[0].message, "unexpected character '{'");
    // the statements are split like commands and keep their lines
    let source = "MATRIX.\nCOMPUTE A = {1, 2;\n 3, 4}.\nPRINT A.\nEND MATRIX.\nFREQUENCIES x.\n";
    let raws = split_commands(source).unwrap();
    let names: Vec<(&str, u32, u32)> = raws
        .iter()
        .map(|r| (r.name.as_str(), r.first_line, r.last_line))
        .collect();
    assert_eq!(
        names,
        [
            ("MATRIX", 1, 1),
            ("COMPUTE", 2, 3),
            ("PRINT", 4, 4),
            ("END MATRIX", 5, 5),
            ("FREQUENCIES", 6, 6)
        ]
    );
    let (parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!((parsed[0].first_line, parsed[0].last_line), (1, 5));
    assert!(matches!(parsed[1].command, Command::Frequencies(_)));
    // a slash ends the expression of a subcommand unless it is in brackets
    let statements = program("READ A /FILE='x' /FIELD=1 TO 9 /SIZE={6 / 2, 3} /MODE=SYMMETRIC.");
    let StatementKind::Read(read) = &statements[0].kind else {
        panic!();
    };
    assert_eq!(shape(read.size.as_ref().unwrap()), "{(6 Div 2),3}");
    assert!(read.symmetric);
}

#[test]
fn help_describes_matrix() {
    let text = help(Some("MATRIX")).unwrap();
    assert!(
        text.contains("END MATRIX") && text.contains("KRONEKER"),
        "{text}"
    );
    assert!(help(Some("END MATRIX")).unwrap().contains("MATRIX starts"));
    assert!(!unsupported_commands().contains(&"MATRIX"));
    assert!(matrix_function_names().contains(&"IDF.SRANGE".to_string()));
    assert_eq!(
        matrix_function("t").map(|f| f.0),
        Some("TRANSPOS".to_string())
    );
}
