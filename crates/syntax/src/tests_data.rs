//! Parser tests of the data management commands.

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

fn var(name: &str) -> VarItem {
    VarItem::Name(name.to_string())
}

fn range(first: &str, last: &str) -> VarItem {
    VarItem::Range(first.to_string(), last.to_string())
}

fn key(vars: Vec<VarItem>, descending: bool) -> SortKey {
    SortKey { vars, descending }
}

#[test]
fn count() {
    use RecodeInput as In;
    let source = "COUNT n = q1 TO q3 (1, 2 THRU 4) plan ('a' 'b')\n / gaps = q1 q2 (MISSING, sysmis, LO THRU -1).";
    assert_eq!(
        one(source),
        Command::Count(vec![
            Count {
                target: "n".into(),
                criteria: vec![
                    (
                        vec![range("q1", "q3")],
                        vec![
                            In::Value(Value::Number(1.0)),
                            In::Range(Bound::Number(2.0), Bound::Number(4.0))
                        ]
                    ),
                    (
                        vec![var("plan")],
                        vec![
                            In::Value(Value::String("a".into())),
                            In::Value(Value::String("b".into()))
                        ]
                    ),
                ],
            },
            Count {
                target: "gaps".into(),
                criteria: vec![(
                    vec![var("q1"), var("q2")],
                    vec![
                        In::Missing,
                        In::Sysmis,
                        In::Range(Bound::Lowest, Bound::Number(-1.0))
                    ]
                )],
            },
        ])
    );

    let error = err("COUNT n = q1 ().");
    assert_eq!(error.message, "unexpected ')'");
    assert_eq!(error.expected, ["a value", "a range", "MISSING", "SYSMIS"]);
    assert_eq!((error.span.line, error.span.column), (1, 15));
    assert_eq!(err("COUNT n = q1 (ELSE).").message, "unexpected word ELSE");
    assert_eq!(err("COUNT n q1 (1).").expected, ["'='"]);
    assert_eq!(err("COUNT n = (1).").expected, ["a variable name"]);
    assert_eq!(err("COUNT n = q1 (1) 5.").message, "unexpected number 5");
}

#[test]
fn sort_cases() {
    assert_eq!(
        one("SORT CASES BY region gender (D) age income (A) id."),
        Command::SortCases(vec![
            key(vec![var("region"), var("gender")], true),
            key(vec![var("age"), var("income")], false),
            key(vec![var("id")], false),
        ])
    );
    assert_eq!(
        one("sort cases q1 to q5 (down) age (UP)."),
        Command::SortCases(vec![
            key(vec![range("q1", "q5")], true),
            key(vec![var("age")], false),
        ])
    );
    let error = err("SORT CASES BY age (X).");
    assert_eq!(error.message, "unexpected word X");
    assert_eq!(error.expected, ["A", "D"]);
    assert_eq!(err("SORT CASES BY.").expected, ["a variable name"]);
    assert_eq!(err("SORT CASES BY age 5.").message, "unexpected number 5");
}

#[test]
fn rename_variables() {
    assert_eq!(
        one("RENAME VARIABLES (a b = b a) (v1 TO v3 = x1 TO x3)."),
        Command::RenameVariables(vec![
            (vec![var("a"), var("b")], vec![var("b"), var("a")]),
            (vec![range("v1", "v3")], vec![range("x1", "x3")]),
        ])
    );
    assert_eq!(
        one("rename var old = new old2 = new2."),
        Command::RenameVariables(vec![
            (vec![var("old")], vec![var("new")]),
            (vec![var("old2")], vec![var("new2")]),
        ])
    );
    let error = err("RENAME VARIABLES a b = c d.");
    assert_eq!(error.message, "unexpected word b");
    assert!(error.hint.unwrap().contains("(old1 old2 = new1 new2)"));
    assert_eq!(
        err("RENAME VARIABLES (a = 1x).").message,
        "invalid number 1x"
    );
    assert_eq!(err("RENAME VARIABLES (a = b.").expected, ["')'"]);
}

#[test]
fn delete_variables_and_weight() {
    assert_eq!(
        one("DELETE VARIABLES tmp q1 TO q3."),
        Command::DeleteVariables(vec![var("tmp"), range("q1", "q3")])
    );
    assert_eq!(err("DELETE VARIABLES.").expected, ["a variable name"]);
    assert_eq!(
        err("DELETE VARIABLES a /b.").expected,
        ["a variable name", "end of command"]
    );

    assert_eq!(one("WEIGHT BY w."), Command::Weight(Some("w".into())));
    assert_eq!(one("weight w."), Command::Weight(Some("w".into())));
    assert_eq!(one("WEIGHT OFF."), Command::Weight(None));
    assert_eq!(one("WEIGHT BY off."), Command::Weight(Some("off".into())));
    assert_eq!(err("WEIGHT BY.").expected, ["a variable name"]);
    assert_eq!(err("WEIGHT BY a b.").message, "unexpected word b");
}

#[test]
fn aggregate() {
    use AggregateFunction as F;
    let source = "AGGREGATE /OUTFILE=* MODE=ADDVARIABLES /BREAK=region gender (D)
  /mean_income 'Mean income' = MEAN(income) /n = N /lo hi = min(age q1)
  /m1 TO m3 = SUM(v1 TO v3).";
    let targets = |names: &[&str]| -> Vec<(VarItem, Option<String>)> {
        names.iter().map(|name| (var(name), None)).collect()
    };
    assert_eq!(
        one(source),
        Command::Aggregate(Aggregate {
            outfile: None,
            add_variables: true,
            breaks: vec![key(vec![var("region"), var("gender")], true)],
            aggregations: vec![
                Aggregation {
                    targets: vec![(var("mean_income"), Some("Mean income".into()))],
                    function: F::Mean,
                    sources: vec![var("income")],
                },
                Aggregation {
                    targets: targets(&["n"]),
                    function: F::N,
                    sources: vec![],
                },
                Aggregation {
                    targets: targets(&["lo", "hi"]),
                    function: F::Min,
                    sources: vec![var("age"), var("q1")],
                },
                Aggregation {
                    targets: vec![(range("m1", "m3"), None)],
                    function: F::Sum,
                    sources: vec![range("v1", "v3")],
                },
            ],
        })
    );
    let Command::Aggregate(to_file) =
        one("AGGREGATE OUTFILE='groups.sav' MODE=REPLACE /nu = NU(age) /missing = NMISS(age).")
    else {
        panic!("not AGGREGATE");
    };
    assert_eq!(to_file.outfile.as_deref(), Some("groups.sav"));
    assert!(!to_file.add_variables && to_file.breaks.is_empty());
    assert_eq!(to_file.aggregations[1].targets, targets(&["missing"]));
    assert_eq!(to_file.aggregations[1].function, F::Nmiss);
}

#[test]
fn aggregate_errors() {
    let error = err("AGGREGATE /OUTFILE=* /BREAK=g /p = PGT(age, 40).");
    assert_eq!(error.message, "aggregate function PGT is not supported");
    assert_eq!((error.span.line, error.span.column), (1, 36));
    assert!(error.expected.contains(&"MEDIAN".to_string()));

    for (source, subcommand) in [
        (
            "AGGREGATE /OUTFILE=* /PRESORTED /BREAK=g /m = MEAN(a).",
            "PRESORTED",
        ),
        (
            "AGGREGATE /OUTFILE=* /MISSING=COLUMNWISE /m = MEAN(a).",
            "MISSING",
        ),
    ] {
        assert_eq!(
            err(source).message,
            format!("subcommand {subcommand} of AGGREGATE is not supported yet")
        );
    }
    assert_eq!(
        err("AGGREGATE /BREAK=g /m = MEAN(a).").expected,
        ["OUTFILE"]
    );
    assert_eq!(
        err("AGGREGATE /OUTFILE=* MODE=FAST /m = N.").expected,
        ["ADDVARIABLES", "REPLACE"]
    );
    assert_eq!(err("AGGREGATE /OUTFILE=* /m = MEAN.").expected, ["'('"]);
    assert_eq!(
        err("AGGREGATE /OUTFILE=* /BREAK=g.").expected,
        ["/target = FUNCTION(variable)"]
    );
    assert_eq!(
        err("AGGREGATE /OUTFILE=* /m MEAN(a).").message,
        "unexpected '('"
    );
    assert_eq!(
        err("AGGREGATE /OUTFILE=* /m = MEAN(a) n = N.").expected,
        ["'/'", "end of command"]
    );
}
