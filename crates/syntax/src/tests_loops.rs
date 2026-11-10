//! Parser tests of DO REPEAT, LOOP, BREAK and VECTOR.

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

fn num(n: f64) -> Expr {
    Expr::Number(n)
}

fn var(name: &str) -> Expr {
    Expr::Variable(name.into())
}

#[test]
fn loop_index_step_and_conditions_in_any_order() {
    let Command::Loop(header) = one("LOOP #i = 2 BY 2 TO n IF (x > 1).") else {
        panic!()
    };
    let index = header.index.unwrap();
    assert_eq!(index.variable, "#i");
    assert_eq!(
        (index.first, index.last, index.by),
        (num(2.0), var("n"), Some(num(2.0)))
    );
    assert!(header.condition.is_some());
    assert_eq!(
        one("LOOP."),
        Command::Loop(LoopHeader {
            index: None,
            condition: None
        })
    );
    assert!(matches!(
        one("LOOP IF x < 5."),
        Command::Loop(LoopHeader {
            index: None,
            condition: Some(_)
        })
    ));
    assert!(matches!(
        one("END LOOP IF x = 5."),
        Command::EndLoop(Some(_))
    ));
    assert_eq!(one("BREAK."), Command::Break);
    assert_eq!(err("LOOP #i = 1.").message, "unexpected end of command");
}

#[test]
fn vector_forms() {
    let Command::Vector(specs) = one("VECTOR v = a TO c / x, y (5, A3) / z (F2 4).") else {
        panic!()
    };
    assert_eq!(specs[0].names, ["v"]);
    assert_eq!(
        specs[0].variables,
        VectorVariables::Existing(VarItem::Range("a".into(), "c".into()))
    );
    assert_eq!(specs[1].names, ["x", "y"]);
    let VectorVariables::New { count, format } = &specs[1].variables else {
        panic!()
    };
    assert_eq!((*count, format.unwrap().kind), (5, FormatType::A));
    assert!(matches!(
        specs[2].variables,
        VectorVariables::New { count: 4, .. }
    ));
    assert_eq!(
        err("VECTOR v = a b.").message,
        "VECTOR needs its variables as a range: first TO last"
    );
}

#[test]
fn an_indexed_target_assigns_a_vector_element() {
    assert_eq!(
        one("COMPUTE v(#i) = 1."),
        Command::VectorAssign {
            condition: None,
            vector: "v".into(),
            index: var("#i"),
            expr: num(1.0),
        }
    );
    assert!(matches!(
        one("IF (x > 1) v(2) = x."),
        Command::VectorAssign {
            condition: Some(_),
            ..
        }
    ));
}

#[test]
fn do_repeat_takes_its_body_up_to_end_repeat() {
    let source = "DO REPEAT a = x TO z / b = 1 TO 3 / c = 'p' -2.5 'q'.\nCOMPUTE a = b.\n\
                  DO REPEAT d = e f.\nCOMPUTE d = 0.\nEND REPEAT.\nEND REPEAT PRINT.\nEXECUTE.";
    let (parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(parsed.len(), 2);
    assert_eq!((parsed[0].first_line, parsed[0].last_line), (1, 6));
    let Command::DoRepeat(repeat) = &parsed[0].command else {
        panic!()
    };
    assert!(repeat.print);
    assert_eq!(repeat.body.len(), 4);
    let values = |n: &[f64]| RepeatList::Values(n.iter().map(|v| Value::Number(*v)).collect());
    assert_eq!(repeat.lists[1], ("b".into(), values(&[1.0, 2.0, 3.0])));
    assert_eq!(
        repeat.lists[2].1,
        RepeatList::Values(vec![
            Value::String("p".into()),
            Value::Number(-2.5),
            Value::String("q".into())
        ])
    );
    // the body is checked with the first item of each list, at its own lines
    let error = err("DO REPEAT a = x y.\nCOMPUTE a = .\nEND REPEAT.");
    assert_eq!((error.span.line, error.span.column), (2, 13));
    assert_eq!(
        err("DO REPEAT a = 1 TO 2 / A = 3 4.\nEND REPEAT.").message,
        "stand-in A is given twice"
    );
}

#[test]
fn substitution_replaces_stand_ins_after_the_command_name() {
    let raw = &split_commands("COMPUTE compute = a + b.").unwrap()[0];
    let a = RepeatItem::Value(Value::Number(-1.0));
    let compute = RepeatItem::Name("x".into());
    let done = substitute(raw, &[("A", &a), ("compute", &compute)]);
    assert_eq!(command_text(&done), "COMPUTE x = - 1 + b");
}
