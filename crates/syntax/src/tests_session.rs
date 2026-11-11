//! Parsing of the session commands: SET, SHOW, PRESERVE, RESTORE, CACHE, CD, DATASET, USE,
//! DISPLAY and LIST.

use super::*;

fn one(source: &str) -> Command {
    let (mut parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    assert_eq!(parsed.len(), 1, "{source}");
    parsed.remove(0).command
}

fn err(source: &str) -> String {
    let (_, errors) = parse(source);
    assert_eq!(errors.len(), 1, "{source}: {errors:?}");
    errors[0].to_string()
}

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect()
}

#[test]
fn set_takes_subcommands_with_or_without_equals_and_slashes() {
    assert_eq!(
        one(
            "SET UNICODE=ON LOCALE='en_UK' /DECIMAL DOT FORMAT=F10.3 MIOUTPUT=[observed pooled] MXWARNS 5 TNUM BOTH."
        ),
        Command::Set(pairs(&[
            ("UNICODE", "ON"),
            ("LOCALE", "en_UK"),
            ("DECIMAL", "DOT"),
            ("FORMAT", "F10.3"),
            ("MIOUTPUT", "[OBSERVED POOLED]"),
            ("MXWARNS", "5"),
            ("TNUMBERS", "BOTH"),
        ]))
    );
    assert_eq!(
        err("SET WOBBLE=1."),
        "line 1, column 5: unknown SET subcommand WOBBLE"
    );
    assert!(err("SET.").contains("expected: a SET subcommand"));
}

#[test]
fn show_preserve_restore_cache_and_cd() {
    assert_eq!(
        one("SHOW LOCALE /N $VARS FORMAT."),
        Command::Show(vec![
            "LOCALE".into(),
            "N".into(),
            "$VARS".into(),
            "FORMAT".into()
        ])
    );
    assert_eq!(one("SHOW."), Command::Show(vec![]));
    assert_eq!(
        err("SHOW NONSENSE."),
        "line 1, column 6: unknown SHOW keyword NONSENSE"
    );
    assert_eq!(one("PRESERVE."), Command::Preserve(vec![]));
    assert_eq!(
        one("PRESERVE LOCALE."),
        Command::Preserve(vec!["LOCALE".into()])
    );
    assert_eq!(
        one("RESTORE LOCALE."),
        Command::Restore(vec!["LOCALE".into()])
    );
    assert_eq!(one("CACHE."), Command::Cache);
    assert_eq!(one("CD '/data/2026'."), Command::Cd("/data/2026".into()));
    assert_eq!(one("CD data."), Command::Cd("data".into()));
}

#[test]
fn file_handle_and_handles_in_place_of_paths() {
    assert_eq!(
        one("FILE HANDLE wave /NAME='w2.sav' /MODE=CHAR /RECFORM=V /LRECL=200 /ENCODING='1257'."),
        Command::FileHandle {
            name: "wave".into(),
            path: "w2.sav".into(),
            encoding: Some("1257".into()),
            ignored: vec!["RECFORM".into(), "LRECL".into()],
        }
    );
    // the old form without a slash
    assert!(matches!(
        one("FILE HANDLE w NAME='x'."),
        Command::FileHandle { .. }
    ));
    assert!(err("FILE HANDLE w /NAME='x' /MODE=BINARY.").contains("binary"));
    assert!(err("FILE HANDLE w /MODE=360 /NAME='x'.").contains("MODE=360"));
    assert!(err("FILE HANDLE w.").contains("NAME"));
    assert!(matches!(one("GET FILE=wave."), Command::Get { file, .. } if file == "wave"));
    assert!(
        matches!(one("SAVE OUTFILE wave."), Command::Save { outfile, .. } if outfile == "wave")
    );
    assert!(matches!(one("INCLUDE lib."), Command::Insert { file, .. } if file == "lib"));
}

#[test]
fn dataset_commands() {
    let d = |c: DatasetCommand| Command::Dataset(c);
    assert_eq!(
        one("DATASET NAME base WINDOW=FRONT."),
        d(DatasetCommand::Name("base".into()))
    );
    assert_eq!(
        one("DATASET ACTIVATE base."),
        d(DatasetCommand::Activate("base".into()))
    );
    assert_eq!(
        one("DATASET COPY men WINDOW=HIDDEN."),
        d(DatasetCommand::Copy("men".into()))
    );
    assert_eq!(
        one("DATASET DECLARE out."),
        d(DatasetCommand::Declare("out".into()))
    );
    assert_eq!(one("DATASET CLOSE *."), d(DatasetCommand::Close(None)));
    assert_eq!(one("DATASET CLOSE ALL."), d(DatasetCommand::CloseAll));
    assert_eq!(
        one("DATASET CLOSE men."),
        d(DatasetCommand::Close(Some("men".into())))
    );
    assert_eq!(one("DATASET DISPLAY."), d(DatasetCommand::Display));
    assert!(err("DATASET ACTIVATE base WINDOW=MINIMIZED.").contains("expected: ASIS, FRONT"));
}

#[test]
fn use_takes_all_or_a_range_of_case_numbers() {
    assert_eq!(one("USE ALL."), Command::Use(None));
    assert_eq!(one("USE THRU."), Command::Use(None));
    assert_eq!(
        one("USE 3 THRU 10 /PERMANENT."),
        Command::Use(Some(UseRange {
            first: Some(3),
            last: Some(10),
            permanent: true
        }))
    );
    assert_eq!(
        one("USE FIRST THRU 5."),
        Command::Use(Some(UseRange {
            first: None,
            last: Some(5),
            permanent: false
        }))
    );
    assert!(err("USE Y 1960.").contains("USE with dates is not supported yet"));
}

#[test]
fn display_and_list_forms() {
    let display = |what: &str, sorted, variables: Option<Vec<VarItem>>| {
        Command::Display(Display {
            what: what.into(),
            sorted,
            variables,
        })
    };
    let x = || Some(vec![VarItem::Name("x".into())]);
    assert_eq!(one("DISPLAY."), display("NAMES", false, None));
    assert_eq!(
        one("DISPLAY DICTIONARY /VARIABLES=x."),
        display("DICTIONARY", false, x())
    );
    assert_eq!(
        one("DISPLAY SORTED LABELS VARIABLES=x."),
        display("LABELS", true, x())
    );
    assert_eq!(one("DISPLAY VARIABLES=x."), display("NAMES", false, x()));
    assert_eq!(
        one("DISPLAY @ATTRIBUTES."),
        display("@ATTRIBUTES", false, None)
    );
    assert_eq!(
        one("DISPLAY FILE LABEL."),
        display("FILE LABEL", false, None)
    );
    assert_eq!(
        one("LIST."),
        Command::List(List {
            variables: None,
            from: 1,
            to: None,
            by: 1,
            numbered: false
        })
    );
    assert_eq!(
        one("LIST a b /CASES=FROM 2 TO 9 BY 3 /FORMAT=NUMBERED SINGLE."),
        Command::List(List {
            variables: Some(vec![VarItem::Name("a".into()), VarItem::Name("b".into())]),
            from: 2,
            to: Some(9),
            by: 3,
            numbered: true
        })
    );
    assert_eq!(
        one("LIST VARIABLES=ALL /CASES 100."),
        Command::List(List {
            variables: Some(vec![VarItem::All]),
            from: 1,
            to: Some(100),
            by: 1,
            numbered: false
        })
    );
}
