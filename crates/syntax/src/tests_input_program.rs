//! Parsing of INPUT PROGRAM, END INPUT PROGRAM, END CASE, END FILE, LEAVE, REREAD,
//! REPEATING DATA, FILE TYPE, RECORD TYPE, END FILE TYPE and DATA LIST END.

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

fn f(kind: FormatType, width: u16, decimals: u8) -> Format {
    Format {
        kind,
        width,
        decimals,
    }
}

#[test]
fn block_commands_and_their_abbreviations() {
    assert_eq!(one("INPUT PROGRAM."), Command::InputProgram);
    assert_eq!(one("INP PROG."), Command::InputProgram);
    assert_eq!(one("END INPUT PROGRAM."), Command::EndInputProgram);
    assert_eq!(one("END CASE."), Command::EndCase);
    assert_eq!(one("END FILE."), Command::EndFile);
    assert_eq!(one("END FILE TYPE."), Command::EndFileType);
    assert!(err("END CASE now.").contains("end of command"));
}

#[test]
fn leave_takes_a_variable_list() {
    assert_eq!(
        one("LEAVE a b TO d."),
        Command::Leave(vec![
            VarItem::Name("a".into()),
            VarItem::Range("b".into(), "d".into())
        ])
    );
    assert!(err("LEAVE.").contains("a variable name"));
}

#[test]
fn reread_takes_a_file_and_a_column_expression() {
    assert_eq!(
        one("REREAD."),
        Command::Reread {
            file: None,
            column: None
        }
    );
    let Command::Reread { file, column } = one("REREAD FILE=update COLUMN = #col + 1.") else {
        panic!()
    };
    assert_eq!(file.as_deref(), Some("update"));
    assert!(column.is_some());
    assert!(one("REREAD COLUMN #COL.") != one("REREAD."));
    assert!(err("REREAD ROW=2.").contains("COLUMN"));
}

#[test]
fn repeating_data_subcommands() {
    let Command::RepeatingData(r) = one(
        "REPEATING DATA STARTS=10-55 /OCCURS=NITEMS /CONTINUED=7-55 /ID=1-5=ORDERID\n\
         /NOTABLE /DATA=ITEM 1-9 (A) QUANTITY 11-13 PRICE (DOLLAR7.2,1X).",
    ) else {
        panic!()
    };
    assert_eq!(r.starts, Place::Column(10));
    assert_eq!(r.ends, Some(Place::Column(55)));
    assert_eq!(r.occurs, Place::Variable("NITEMS".into()));
    assert_eq!(r.continued, Some((7, Some(55))));
    assert_eq!(r.id, Some((1, 5, "ORDERID".into())));
    assert_eq!(r.fields.len(), 3);
    assert_eq!(r.fields[2].column, 14);
    assert_eq!(r.fields[2].format, f(FormatType::Dollar, 7, 2));
    // the skip after the last variable belongs to the group
    assert_eq!(r.group_length, 21);
    let Command::RepeatingData(r) =
        one("REPEATING DATA STARTS=FIRST /OCCURS=1 /LENGTH=10 /CONTINUED /DATA=MAKE 1-8 (A).")
    else {
        panic!()
    };
    assert_eq!(r.starts, Place::Variable("FIRST".into()));
    assert_eq!(r.length, Some(Place::Column(10)));
    assert_eq!(r.continued, Some((1, None)));
    assert_eq!(r.group_length, 8);
    assert!(err("REPEATING DATA OCCURS=2 /DATA=x 1.").contains("STARTS"));
    assert!(err("REPEATING DATA STARTS=1 /DATA=x 1 /OCCURS=2.").contains("past RECORDS"));
    assert!(err("REPEATING DATA STARTS=1 /OCCURS=2 /ID=1-2=x /DATA=y 3.").contains("CONTINUED"));
}

#[test]
fn file_type_subcommands() {
    let Command::FileType(t) = one(
        "FILE TYPE GROUPED FILE=HUBDATA RECORD=#RECID 80 CASE=ID 1-5 (N) WILD=NOWARN \
         DUPLICATE=NOWARN MISSING=WARN ORDERED=NO.",
    ) else {
        panic!()
    };
    assert_eq!(t.kind, FileKind::Grouped);
    assert_eq!(t.file.as_deref(), Some("HUBDATA"));
    assert_eq!(t.record.name.as_deref(), Some("#RECID"));
    assert_eq!(t.record.column, 80);
    assert_eq!(t.record.format, f(FormatType::F, 1, 0));
    let case = t.case.unwrap();
    assert_eq!((case.column, case.format), (1, f(FormatType::N, 5, 0)));
    assert_eq!(t.wild, Some(false));
    assert_eq!(t.duplicate, Some(Duplicate::NoWarn));
    assert_eq!(t.missing, Some(true));
    assert_eq!(t.ordered, Some(false));
    let Command::FileType(t) = one("FILE TYPE NESTED RECORD=6 CASE=ACCID 1-4 DUPLICATE=CASE.")
    else {
        panic!()
    };
    assert_eq!(t.record.name, None);
    assert_eq!(t.duplicate, Some(Duplicate::Case));
    let Command::FileType(t) = one("FILE TYPE MIXED RECORD=RECID 1-2 (A).") else {
        panic!()
    };
    assert_eq!(t.record.format, f(FormatType::A, 2, 0));
    assert!(err("FILE TYPE MIXED RECORD=r 1 CASE=c 2.").contains("grouped and nested"));
    assert!(err("FILE TYPE GROUPED RECORD=r 1.").contains("needs CASE"));
    assert!(err("FILE TYPE GROUPED RECORD=r 1 CASE=c 2 DUPLICATE=CASE.").contains("nested"));
    assert!(err("FILE TYPE NESTED RECORD=r 1 ORDERED=YES.").contains("grouped"));
    assert!(err("FILE TYPE MIXED.").contains("RECORD"));
    assert!(err("FILE TYPE ODD RECORD=1.").contains("MIXED"));
}

#[test]
fn record_type_values_and_options() {
    let Command::RecordType(r) = one("RECORD TYPE 21,22 23 SKIP.") else {
        panic!()
    };
    assert_eq!(
        r.values,
        [
            Value::Number(21.0),
            Value::Number(22.0),
            Value::Number(23.0)
        ]
    );
    assert!(r.skip);
    let Command::RecordType(r) = one("RECORD TYPE OTHER SKIP.") else {
        panic!()
    };
    assert!(r.values.is_empty() && r.skip);
    let Command::RecordType(r) =
        one("RECORD TYPE 'A' CASE=75-79 DUPLICATE=NOWARN MISSING=NOWARN SPREAD=NO.")
    else {
        panic!()
    };
    assert_eq!(r.values, [Value::String("A".into())]);
    assert_eq!(r.case, Some((75, 79)));
    assert_eq!(r.duplicate, Some(Duplicate::NoWarn));
    assert_eq!(r.missing, Some(false));
    assert!(!r.spread);
    assert!(err("RECORD TYPE.").contains("OTHER"));
    assert!(err("RECORD TYPE 1 DUPLICATE=CASE.").contains("FILE TYPE NESTED"));
}

#[test]
fn data_list_end_and_records_without_variables() {
    let Command::DataList(list) = one("DATA LIST FILE=INCOME END=#EOF NOTABLE / NAME 1-10(A).")
    else {
        panic!()
    };
    assert_eq!(list.end.as_deref(), Some("#EOF"));
    let Command::DataList(list) = one("DATA LIST NOTABLE/.") else {
        panic!()
    };
    assert!(list.fields.is_empty());
    assert!(err("DATA LIST LIST END=#e /x.").contains("fixed-format"));
    // a repeated group of formats
    let Command::DataList(list) = one("DATA LIST /ORDER 1-4 #X1 TO #X4 (1X,2(F3.0,F2.0,1X)).")
    else {
        panic!()
    };
    let columns: Vec<u32> = list.fields.iter().map(|f| f.column).collect();
    assert_eq!(columns, [1, 6, 9, 12, 15]);
}
