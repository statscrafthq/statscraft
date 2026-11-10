//! Parser tests of inline data and DATA LIST.

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

fn data_list(source: &str) -> DataList {
    match one(source) {
        Command::DataList(list) => list,
        other => panic!("{other:?}"),
    }
}

/// Each field as "name FORMAT record:column".
fn fields(list: &DataList) -> Vec<String> {
    let format = |f: &Format| match f.decimals {
        0 => format!("{}{}", f.kind.name(), f.width),
        d => format!("{}{}.{d}", f.kind.name(), f.width),
    };
    let text =
        |f: &DataField| format!("{} {} {}:{}", f.name, format(&f.format), f.record, f.column);
    list.fields.iter().map(text).collect()
}

#[test]
fn inline_data_lines_are_data_with_their_own_line_numbers() {
    let source =
        "DATA LIST LIST /a b.\nBEGIN DATA\n1 2.\n* not a comment\n\nEND DATA.\nCOMPUTE c = a.";
    let (parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(parsed.len(), 3);
    assert_eq!(
        parsed[1].command,
        Command::BeginData(vec![
            (3, "1 2.".into()),
            (4, "* not a comment".into()),
            (5, String::new())
        ])
    );
    assert_eq!((parsed[1].first_line, parsed[1].last_line), (2, 6));
    assert_eq!(parsed[2].first_line, 7);
    // BEGIN DATA ends a command that has no period yet; its own period is optional
    let (parsed, errors) = parse("DATA LIST FREE /x\nbegin data.\n1\nend   data\nEXECUTE.");
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(parsed.len(), 3);
    assert_eq!(parsed[1].command, Command::BeginData(vec![(3, "1".into())]));
}

#[test]
fn inline_data_errors() {
    let error = err("DATA LIST FREE /x.\nBEGIN DATA\n1 2\n");
    assert_eq!(error.message, "BEGIN DATA without END DATA");
    assert_eq!(error.span.line, 2);
    assert_eq!(err("END DATA.").message, "END DATA without BEGIN DATA");
}

#[test]
fn a_line_range_can_select_inline_data_whole_but_not_cut_it() {
    let source = "DATA LIST FREE /x.\nBEGIN DATA\n1\n2\nEND DATA.";
    let all = split_commands(source).unwrap();
    assert_eq!(select_lines(&all, 1, 5).unwrap().len(), 2);
    let error = select_lines(&all, 1, 3).unwrap_err();
    assert!(
        error.message.contains("cuts through command BEGIN DATA"),
        "{}",
        error.message
    );
}

#[test]
fn data_list_fixed_columns_and_formats() {
    let list = data_list(
        "DATA LIST FILE='x.txt' RECORDS=2 NOTABLE\n /1 id 1-3 sex 5 (A) op1 TO op3 10-15\n rate 16-20 (DOLLAR,2) /2 name n2 n3 (T3, A4, 1X, 2F2.1) dt 20-30 (ADATE).",
    );
    assert_eq!(list.file.as_deref(), Some("x.txt"));
    assert_eq!(list.layout, DataLayout::Fixed { records: 2 });
    assert_eq!(
        fields(&list),
        [
            "id F3 1:1",
            "sex A1 1:5",
            "op1 F2 1:10",
            "op2 F2 1:12",
            "op3 F2 1:14",
            "rate DOLLAR5.2 1:16",
            "name A4 2:3",
            "n2 F2.1 2:8",
            "n3 F2.1 2:10",
            "dt ADATE11 2:20",
        ]
    );
    // a record number may skip records; a format list continues after the last column
    let list = data_list("DATA LIST RECORDS=3 /2 a 1-2 b (F3) /3 c 1-4 (2).");
    assert_eq!(fields(&list), ["a F2 2:1", "b F3 2:3", "c F4.2 3:1"]);
}

#[test]
fn data_list_free_formats() {
    let list =
        data_list("DATA LIST LIST (',' TAB) SKIP=1 /a b * s (A5) c d (F3.1) e f g (2F2, A1) h.");
    assert_eq!(list.layout, DataLayout::List);
    assert_eq!(list.delimiters, [',', '\t']);
    assert_eq!(list.skip, 1);
    assert_eq!(
        fields(&list),
        [
            "a F8.2 1:0",
            "b F8.2 1:0",
            "s A5 1:0",
            "c F3.1 1:0",
            "d F3.1 1:0",
            "e F2 1:0",
            "f F2 1:0",
            "g A1 1:0",
            "h F8.2 1:0",
        ]
    );
    let error = err("DATA LIST FREE /a b c (F2, F3).");
    assert!(
        error.message.starts_with("3 variables but 2 formats"),
        "{}",
        error.message
    );
}

#[test]
fn data_list_errors() {
    let message = |source: &str| err(source).message;
    assert_eq!(
        message("DATA LIST /a b 1-3."),
        "columns 1-3 cannot be shared equally by 2 variables"
    );
    assert_eq!(
        message("DATA LIST /a (F2, F2)."),
        "more formats than variables"
    );
    assert_eq!(
        message("DATA LIST /a b (F2)."),
        "fewer formats than variables: give one format per variable"
    );
    assert_eq!(
        message("DATA LIST /a 1 /b 2."),
        "record 2 is past RECORDS=1"
    );
    assert_eq!(
        message("DATA LIST LIST /a (RB8)."),
        "DATA LIST cannot read format RB yet"
    );
    assert_eq!(
        message("DATA LIST /a 1 a 2."),
        "variable a is defined twice"
    );
    assert_eq!(
        message("DATA LIST LIST RECORDS=2 /a."),
        "RECORDS is for fixed-format data only"
    );
}

#[test]
fn display_formats_make_room_for_punctuation() {
    let shown = |text: &str| {
        let f = crate::data_list::display_format(crate::commands::format(text).unwrap());
        format!("{}{}.{}", f.kind.name(), f.width, f.decimals)
    };
    assert_eq!(shown("F5.2"), "F6.2");
    assert_eq!(shown("N3"), "F3.0");
    assert_eq!(shown("E6.1"), "E10.3");
    assert_eq!(shown("DOLLAR9.2"), "DOLLAR13.2");
    assert_eq!(shown("COMMA5.1"), "COMMA7.1");
    assert_eq!(shown("DOT8.1"), "DOT11.1");
    assert_eq!(shown("PCT5.2"), "PCT7.2");
    assert_eq!(shown("ADATE10"), "ADATE10.0");
    // the F8.2 of a variable without a format is shown as it is
    let list = data_list("DATA LIST FREE /a * b (F8.2).");
    assert_eq!(list.fields[0].display, list.fields[0].format);
    assert_eq!(list.fields[1].display.width, 9);
}
