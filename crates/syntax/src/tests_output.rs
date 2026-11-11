//! Parsing of TITLE, SUBTITLE, ECHO, NEW FILE, CLEAR TRANSFORMATIONS, ERASE, SYSFILE INFO,
//! PERMISSIONS, HOST, DATE, PRINT, PRINT EJECT, PRINT SPACE, WRITE, PRINT FORMATS,
//! WRITE FORMATS and XSAVE.

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

fn names(names: &[&str]) -> Vec<VarItem> {
    names.iter().map(|n| VarItem::Name(n.to_string())).collect()
}

fn print(source: &str) -> Print {
    match one(source) {
        Command::Print(print) => *print,
        other => panic!("{source}: {other:?}"),
    }
}

fn f(kind: FormatType, width: u16, decimals: u8) -> Format {
    Format {
        kind,
        width,
        decimals,
    }
}

#[test]
fn titles_take_free_text_or_a_quoted_string() {
    let title = |text: &str, subtitle| Command::Title {
        text: text.into(),
        subtitle,
    };
    assert_eq!(one("TITLE My Title."), title("My Title", false));
    assert_eq!(one("TITLE \"Fred's Title\"."), title("Fred's Title", false));
    assert_eq!(
        one("TITLE 'Fred''s \"Quoted\" Title'."),
        title("Fred's \"Quoted\" Title", false)
    );
    assert_eq!(one("SUBTITLE ' '."), title(" ", true));
    assert_eq!(
        one("SUBT Children's shoes."),
        title("Children's shoes", true)
    );
    assert_eq!(
        one("ECHO 'one ' + \"piece\"."),
        Command::Echo("one piece".into())
    );
    assert!(err("ECHO text.").contains("expected: a quoted text"));
}

#[test]
fn file_commands_need_their_keywords() {
    assert_eq!(one("NEW FILE."), Command::NewFile);
    assert_eq!(one("CLEAR TRANSFORMATIONS."), Command::ClearTransformations);
    assert_eq!(one("ERASE FILE 'a.txt'."), Command::Erase("a.txt".into()));
    assert_eq!(one("ERASE FILE='a.txt'."), Command::Erase("a.txt".into()));
    assert!(err("ERASE 'a.txt'.").contains("expected: FILE"));
    assert_eq!(
        one("SYSFILE INFO 'x.sav'."),
        Command::SysfileInfo("x.sav".into())
    );
    assert_eq!(
        one("SYSFILE INFO FILE='x.sav'."),
        Command::SysfileInfo("x.sav".into())
    );
    assert!(err("SYSFILE INFO 'x.sav' PASSWORD='p'.").contains("encrypted"));
    assert_eq!(
        one("PERMISSIONS FILE='x.sav' /PERMISSIONS READONLY."),
        Command::Permissions {
            file: "x.sav".into(),
            read_only: true
        }
    );
    assert_eq!(
        one("PERMISSIONS FILE='x.sav' /PERMISSIONS=WRITEABLE."),
        Command::Permissions {
            file: "x.sav".into(),
            read_only: false
        }
    );
    assert!(err("PERMISSIONS FILE='x.sav' /PERMISSIONS OPEN.").contains("READONLY"));
    assert_eq!(
        one("HOST COMMAND=['dir' 'echo hi'] TIMELIMIT=5."),
        Command::Host(vec!["dir".into(), "echo hi".into()])
    );
    assert!(err("HOST COMMAND='dir'.").contains("expected: '['"));
}

#[test]
fn date_keywords_abbreviate_to_one_letter_and_minute_to_two() {
    let date = |source: &str| match one(source) {
        Command::Date(spec) => spec,
        other => panic!("{other:?}"),
    };
    assert_eq!(
        date("DATE Y 1960 M."),
        DateSpec {
            keywords: vec![
                (DateUnit::Year, Some(1960.0), None),
                (DateUnit::Month, None, None)
            ],
            by: None
        }
    );
    assert_eq!(
        date("DATE DAY, HOUR 1, 24 BY 2.").keywords,
        vec![
            (DateUnit::Day, None, None),
            (DateUnit::Hour, Some(1.0), Some(24.0))
        ]
    );
    assert_eq!(date("DATE DAY HOUR 1 24 BY 2.").by, Some(2.0));
    assert_eq!(date("DATE MI.").keywords[0].0, DateUnit::Minute);
    assert_eq!(date("DATE MO.").keywords[0].0, DateUnit::Month);
    assert_eq!(
        date("DATE OBS 9 17.").keywords[0],
        (DateUnit::Obs, Some(9.0), Some(17.0))
    );
    // a repeated keyword counts at its last place
    assert_eq!(
        date("DATE M 3 Y M 5.").keywords,
        vec![
            (DateUnit::Year, None, None),
            (DateUnit::Month, Some(5.0), None)
        ]
    );
    assert!(err("DATE Y M W.").contains("cannot go with"));
    assert!(err("DATE OBS 3.").contains("OBS needs"));
    assert!(err("DATE C 1 5.").contains("CYCLE takes"));
    assert!(err("DATE BY 2.").contains("expected: a DATE keyword"));
    assert!(err("DATE M BY 2 Y.").contains("BY goes after"));
}

#[test]
fn print_reads_subcommands_then_lines_of_items() {
    let p = print("PRINT / MOHIRED YRHIRED DEPT SALARY NAME.");
    assert_eq!(p.kind, PrintKind::Print);
    assert_eq!(
        p.lines,
        vec![vec![PrintItem::Variables(
            names(&["MOHIRED", "YRHIRED", "DEPT", "SALARY", "NAME"]),
            None
        )]]
    );
    let p = print(
        "PRINT / TENURE (F2.0) ' ' MOHIRED YRHIRED DEPT *\n\
         SALARY85 TO SALARY88 (4(DOLLAR8,1X)) NAME.",
    );
    let dollar = FormatItem::Format(f(FormatType::Dollar, 8, 0));
    assert_eq!(
        p.lines,
        vec![vec![
            PrintItem::Variables(
                names(&["TENURE"]),
                Some(Placement::Formats(vec![FormatItem::Format(f(
                    FormatType::F,
                    2,
                    0
                ))]))
            ),
            PrintItem::Text {
                text: " ".into(),
                columns: None
            },
            PrintItem::Variables(names(&["MOHIRED", "YRHIRED", "DEPT"]), None),
            PrintItem::Variables(
                vec![VarItem::Range("SALARY85".into(), "SALARY88".into())],
                Some(Placement::Formats(vec![
                    dollar,
                    FormatItem::Skip(1),
                    dollar,
                    FormatItem::Skip(1),
                    dollar,
                    FormatItem::Skip(1),
                    dollar,
                    FormatItem::Skip(1)
                ]))
            ),
            PrintItem::Variables(names(&["NAME"]), None),
        ]]
    );
    let p = print(
        "PRINT EJECT OUTFILE='o.txt' TABLE /'NAME' 1 'DEPT' 25-28 MOHIRED 30-31 (F,1) x (T35,DOLLAR8).",
    );
    assert_eq!(p.kind, PrintKind::PrintEject);
    assert_eq!(p.outfile.as_deref(), Some("o.txt"));
    assert!(p.table);
    assert_eq!(
        p.lines[0],
        vec![
            PrintItem::Text {
                text: "NAME".into(),
                columns: Some((1, None))
            },
            PrintItem::Text {
                text: "DEPT".into(),
                columns: Some((25, Some(28)))
            },
            PrintItem::Variables(
                names(&["MOHIRED"]),
                Some(Placement::Columns {
                    first: 30,
                    last: 31,
                    kind: Some(FormatType::F),
                    decimals: 1
                })
            ),
            PrintItem::Variables(
                names(&["x"]),
                Some(Placement::Formats(vec![
                    FormatItem::Tab(35),
                    FormatItem::Format(f(FormatType::Dollar, 8, 0))
                ]))
            ),
        ]
    );
    let p = print("PRINT RECORDS=3 /1 a b /2 c /3.");
    assert_eq!(p.records, Some(3));
    assert_eq!(p.lines.len(), 3);
    assert_eq!(p.lines[2], vec![]);
    let p = print("WRITE OUTFILE=out ENCODING='UTF16LE' BOM=NO /ALL.");
    assert_eq!(p.kind, PrintKind::Write);
    assert_eq!(p.outfile.as_deref(), Some("out"));
    assert!(p.no_bom);
    assert_eq!(
        p.lines,
        vec![vec![PrintItem::Variables(vec![VarItem::All], None)]]
    );
    assert!(err("PRINT a b.").contains("'/' and the variables"));
    assert!(err("PRINT BOM=NO /a.").contains("expected: OUTFILE"));
    assert!(err("PRINT /a (PIB2).").contains("not supported yet"));
    assert!(err("PRINT /a 5-3.").contains("comes before"));
}

#[test]
fn print_space_formats_and_xsave() {
    assert_eq!(
        one("PRINT SPACE."),
        Command::PrintSpace(PrintSpace {
            outfile: None,
            encoding: None,
            lines: None
        })
    );
    match one("PRINT SPACE OUTFILE='o.txt' #n + 1.") {
        Command::PrintSpace(space) => {
            assert_eq!(space.outfile.as_deref(), Some("o.txt"));
            assert!(space.lines.is_some());
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(
        one("PRINT FORMATS a (F8.2) / b c (DOLLAR8)."),
        Command::PrintFormats(vec![
            (names(&["a"]), f(FormatType::F, 8, 2)),
            (names(&["b", "c"]), f(FormatType::Dollar, 8, 0)),
        ])
    );
    assert_eq!(
        one("WRITE FORMATS a (F4)."),
        Command::WriteFormats(vec![(names(&["a"]), f(FormatType::F, 4, 0))])
    );
    match one(
        "XSAVE /KEEP a b /OUTFILE='x.sav' /RENAME (a=z) /MAP /ZCOMPRESSED /PERMISSIONS=READONLY.",
    ) {
        Command::Xsave(x) => {
            assert_eq!(x.outfile, "x.sav");
            assert_eq!(x.options.len(), 2);
            assert!(x.map);
            assert_eq!(x.compression, Some(Compression::ZCompressed));
            assert_eq!(x.read_only, Some(true));
        }
        other => panic!("{other:?}"),
    }
    assert!(err("XSAVE /KEEP a.").contains("XSAVE needs OUTFILE"));
    assert!(err("XSAVE OUTFILE='x' /WHAT.").contains("expected: OUTFILE"));
}
