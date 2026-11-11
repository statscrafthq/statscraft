//! Parser tests of the restructuring commands: VARSTOCASES, CASESTOVARS, FLIP, SORT VARIABLES
//! and UPDATE.

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

fn new(name: &str, label: Option<&str>) -> NewVariable {
    NewVariable {
        name: name.into(),
        label: label.map(str::to_string),
    }
}

#[test]
fn vars_to_cases() {
    let Command::VarsToCases(command) = one(
        "VARSTOCASES /MAKE score 'Score' FROM s1 TO s4 /MAKE w s5 s6 s7 s8\n\
         /INDEX=a 'Factor A' (2) b(2) /ID=person /NULL=KEEP /COUNT=n 'Rows' /KEEP=ALL.",
    ) else {
        panic!("not VARSTOCASES")
    };
    let s1_to_s4 = VarItem::Range("s1".into(), "s4".into());
    assert_eq!(
        command.makes[0],
        (new("score", Some("Score")), vec![s1_to_s4])
    );
    assert_eq!(command.makes[1].1.len(), 4);
    let index = |name, label, levels| VarsToCasesIndex {
        variable: new(name, label),
        kind: IndexKind::Levels(levels),
    };
    assert_eq!(
        command.index,
        [index("a", Some("Factor A"), 2), index("b", None, 2)]
    );
    assert_eq!(command.id, Some(new("person", None)));
    assert!(command.keep_null);
    assert_eq!(command.count, Some(new("n", Some("Rows"))));
    assert_eq!(command.subset, Some(Subset::Keep(vec![VarItem::All])));

    let Command::VarsToCases(names) = one("VARS /MAKE v FROM a b /INDEX=i (v) /DROP=x.") else {
        panic!("not VARSTOCASES")
    };
    assert_eq!(names.index[0].kind, IndexKind::Names("v".into()));
    assert!(!names.keep_null);
    assert_eq!(names.subset, Some(Subset::Drop(vec![var("x")])));

    assert_eq!(
        err("VARSTOCASES /ID=i.").message,
        "VARSTOCASES needs at least one /MAKE"
    );
    assert_eq!(
        err("VARSTOCASES /MAKE v FROM a /ID=i /ID=j.").message,
        "VARSTOCASES takes /ID only once"
    );
    assert_eq!(
        err("VARSTOCASES /MAKE v FROM a /KEEP=x /DROP=y.").message,
        "VARSTOCASES takes one /KEEP or one /DROP, not both or two"
    );
    assert!(
        err("VARSTOCASES /MAKE v FROM a b /INDEX=i j(2).")
            .message
            .contains("each need their number of levels")
    );
    assert_eq!(
        err("VARSTOCASES /MAKE v FROM a /NULL=SOME.").expected,
        ["DROP", "KEEP"]
    );
}

#[test]
fn cases_to_vars() {
    assert_eq!(
        one("CASESTOVARS."),
        Command::CasesToVars(CasesToVars {
            id: Vec::new(),
            index: Vec::new(),
            fixed: Vec::new(),
            autofix: true,
            indicators: None,
            count: None,
            rename: Vec::new(),
            separator: ".".into(),
            group_by_index: false,
            drop: Vec::new(),
        })
    );
    let Command::CasesToVars(command) = one(
        "CASESTOVARS /ID=id /INDEX=month /FIXED=age /AUTOFIX=NO /VIND ROOT=has\n\
         /COUNT=n 'Rows' /RENAME bps=sys bpd=dia /SEPARATOR='_' /GROUPBY=INDEX /DROP=x.",
    ) else {
        panic!("not CASESTOVARS")
    };
    assert_eq!(
        (command.id, command.index),
        (vec![var("id")], vec![var("month")])
    );
    assert_eq!(command.fixed, vec![var("age")]);
    assert!(!command.autofix && command.group_by_index);
    assert_eq!(command.indicators.as_deref(), Some("has"));
    assert_eq!(command.count, Some(new("n", Some("Rows"))));
    assert_eq!(
        command.rename,
        [("bps".into(), "sys".into()), ("bpd".into(), "dia".into())]
    );
    assert_eq!(
        (command.separator.as_str(), command.drop),
        ("_", vec![var("x")])
    );
    let Command::CasesToVars(vind) = one("CASESTOVARS /VIND /SEP=''.") else {
        panic!("not CASESTOVARS")
    };
    assert_eq!(
        (vind.indicators.as_deref(), vind.separator.as_str()),
        (Some("ind"), "")
    );

    assert_eq!(
        err("CASESTOVARS /ID=a /ID=b.").message,
        "CASESTOVARS takes /ID only once"
    );
    assert_eq!(
        err("CASESTOVARS /GROUPBY=CASE.").expected,
        ["VARIABLE", "INDEX"]
    );
}

#[test]
fn flip_and_sort_variables() {
    assert_eq!(
        one("FLIP."),
        Command::Flip(Flip {
            variables: None,
            newnames: None
        })
    );
    assert_eq!(
        one("FLIP VARIABLES=a TO c /NEWNAMES=name."),
        Command::Flip(Flip {
            variables: Some(vec![VarItem::Range("a".into(), "c".into())]),
            newnames: Some("name".into())
        })
    );
    let sort = |key, descending| Command::SortVariables { key, descending };
    assert_eq!(
        one("SORT VARIABLES BY NAME (A)."),
        sort(VariableKey::Name, false)
    );
    assert_eq!(
        one("SORT VAR ali (DOWN)."),
        sort(VariableKey::Alignment, true)
    );
    assert_eq!(
        one("SORT VARIABLES ATTRIBUTE Source (D)."),
        sort(VariableKey::Attribute("Source".into()), true)
    );
    assert!(
        err("SORT VARIABLES BY SIZE.")
            .expected
            .contains(&"ATTRIBUTE name".to_string())
    );
}

#[test]
fn update() {
    let Command::Update(update) = one(
        "UPDATE FILE='master.sav' /RENAME=(a = b) /IN=old /FILE=* /IN=new /BY id /MAP /KEEP=id b.",
    ) else {
        panic!("not UPDATE")
    };
    assert_eq!(update.inputs.len(), 2);
    assert_eq!(update.inputs[0].file.as_deref(), Some("master.sav"));
    assert_eq!(update.inputs[0].flag.as_deref(), Some("old"));
    assert_eq!(update.inputs[1].flag.as_deref(), Some("new"));
    assert_eq!(update.by, vec![var("id")]);
    assert_eq!(
        update.subsets,
        [Subset::Map, Subset::Keep(vec![var("id"), var("b")])]
    );
    assert_eq!(
        err("UPDATE FILE='a.sav' /FILE=*.").message,
        "UPDATE needs /BY key variables"
    );
    assert_eq!(
        err("UPDATE FILE='a.sav' /BY id.").message,
        "UPDATE needs a second /FILE"
    );
    // UPDATE makes one case per key, so it has no FIRST or LAST
    assert_eq!(
        err("UPDATE FILE='a.sav' /FILE=* /BY id /FIRST=f.").message,
        "unknown subcommand FIRST of UPDATE"
    );
}
