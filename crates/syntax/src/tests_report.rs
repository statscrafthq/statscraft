//! Parsing of MATRIX DATA, the OUTPUT commands, REPORT and FINISH.

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

fn matrix(source: &str) -> MatrixData {
    match one(source) {
        Command::MatrixData(m) => *m,
        other => panic!("{source}: {other:?}"),
    }
}

fn report(source: &str) -> Report {
    match one(source) {
        Command::Report(r) => *r,
        other => panic!("{source}: {other:?}"),
    }
}

fn export(source: &str) -> OutputExport {
    match one(source) {
        Command::Output(OutputCommand::Export(e)) => *e,
        other => panic!("{source}: {other:?}"),
    }
}

#[test]
fn matrix_data_reads_its_subcommands() {
    let m = matrix("MATRIX DATA VARIABLES=ROWTYPE_ v1 TO v3 /FORMAT=UPPER NODIAGONAL FREE.");
    assert_eq!(m.variables, ["ROWTYPE_", "v1", "v2", "v3"]);
    assert_eq!(m.continuous(), ["v1", "v2", "v3"]);
    assert!(m.explicit() && m.free && !m.diagonal);
    assert_eq!(m.shape, MatrixShape::Upper);

    let m = matrix(
        "MATRIX DATA VARIABLES=F1 VAR1 TO VAR3 /FACTORS=F1 /CELLS=2 /CONTENTS=(MEAN SD N) CORR.",
    );
    assert_eq!(m.factors, ["F1"]);
    assert_eq!(m.cells, Some(2));
    assert_eq!(
        m.contents,
        [
            (true, vec![RowType::Mean, RowType::StdDev, RowType::NVector]),
            (false, vec![RowType::Corr])
        ]
    );
    // abbreviations and synonyms; FILE first without a slash
    let m = matrix("MATRIX DATA FILE=rawmtx /VARIABLES=a b /CONT=MEA STD N_S COR /N=99 /SPLIT=s.");
    assert_eq!(m.file.as_deref(), Some("rawmtx"));
    assert_eq!(
        m.contents.iter().map(|(_, k)| k[0]).collect::<Vec<_>>(),
        [
            RowType::Mean,
            RowType::StdDev,
            RowType::NScalar,
            RowType::Corr
        ]
    );
    assert_eq!((m.n, m.split.clone()), (Some(99), vec!["s".to_string()]));
    assert_eq!(RowType::from_name("n_v"), Some(RowType::NVector));
    assert_eq!(RowType::from_name("'N'"), None);
    assert_eq!(RowType::from_name("N"), Some(RowType::NVector));
    assert_eq!(RowType::NScalar.value(), "N");
}

#[test]
fn matrix_data_errors() {
    assert!(err("MATRIX DATA /FORMAT=FULL.").contains("needs VARIABLES"));
    assert!(err("MATRIX DATA VARIABLES=a b /FORMAT=FULL NODIAGONAL.").contains("FULL cannot"));
    assert!(err("MATRIX DATA VARIABLES=a VARNAME_.").contains("VARNAME_ cannot be named"));
    assert!(err("MATRIX DATA VARIABLES=a b f /FACTORS=f.").contains("must come before"));
    assert!(err("MATRIX DATA VARIABLES=f a /FACTORS=f.").contains("needs CELLS"));
    assert!(
        err("MATRIX DATA VARIABLES=f a /FACTORS=f /CELLS=2 /CONTENTS=MEAN CORR.")
            .contains("in parentheses")
    );
    assert!(err("MATRIX DATA VARIABLES=ROWTYPE_ a /SPLIT=s.").contains("not on VARIABLES"));
    assert!(err("MATRIX DATA SPLIT=s /VARIABLES=a.").contains("must follow VARIABLES"));
    assert!(err("MATRIX DATA VARIABLES=a /CONTENTS=MEAN BOGUS.").contains("unexpected word"));
}

#[test]
fn output_document_commands() {
    assert_eq!(
        one("OUTPUT NEW NAME=women."),
        Command::Output(OutputCommand::New(Some("women".into())))
    );
    assert_eq!(
        one("OUTPUT NEW TYPE=VIEWER."),
        Command::Output(OutputCommand::New(None))
    );
    assert_eq!(
        one("OUTPUT NAME men."),
        Command::Output(OutputCommand::Name("men".into()))
    );
    assert_eq!(
        one("OUTPUT NAME NAME=men."),
        Command::Output(OutputCommand::Name("men".into()))
    );
    assert_eq!(
        one("OUTPUT ACTIVATE NAME=men."),
        Command::Output(OutputCommand::Activate("men".into()))
    );
    assert_eq!(
        one("OUTPUT CLOSE *."),
        Command::Output(OutputCommand::Close(None))
    );
    assert_eq!(
        one("OUTPUT CLOSE ALL."),
        Command::Output(OutputCommand::CloseAll)
    );
    assert_eq!(
        one("OUTPUT CLOSE NAME=men."),
        Command::Output(OutputCommand::Close(Some("men".into())))
    );
    assert_eq!(
        one("OUTPUT DISPLAY."),
        Command::Output(OutputCommand::Display)
    );
    assert!(err("OUTPUT NAME all.").contains("reserved word"));
    assert!(err("OUTPUT NEW NAME=a NAME=b.").contains("only once"));
}

#[test]
fn output_export_targets() {
    let e = export(
        "OUTPUT EXPORT NAME=Region1 /CONTENTS EXPORT=VISIBLE LAYERS=ALL\n\
         /XLSX DOCUMENTFILE='r.xlsx' OPERATION=MODIFYSHEET SHEET='Region 1' LOCATION=STARTCELL('B2')\n\
         /HTML DOCUMENTFILE='r.htm' NOTESCAPTIONS=NO IMAGEFORMAT=PNG STYLING=NO\n\
         /TEXT DOCUMENTFILE='r.txt' ENCODING=UTF16 SEPARATOR=TAB COLUMNWIDTH=10 ROWBORDER='='\n\
         /CSV DOCUMENTFILE='r.csv' /PNG IMAGEROOT='g' PERCENTSIZE=50.",
    );
    assert_eq!(e.name.as_deref(), Some("Region1"));
    assert_eq!(
        e.targets,
        [
            ExportTarget::Xlsx {
                file: "r.xlsx".into(),
                operation: SheetOperation::ModifySheet,
                sheet: Some("Region 1".into()),
                location: SheetLocation::StartCell(1, 1),
                notes: true,
            },
            ExportTarget::Html {
                file: "r.htm".into(),
                notes: false,
                styling: false,
            },
            ExportTarget::Text {
                file: "r.txt".into(),
                notes: true,
                encoding: TextEncoding::Utf16,
                column_width: Some(10),
                tab: true,
                row_border: '=',
                column_border: '|',
            },
            ExportTarget::Csv {
                file: "r.csv".into(),
                notes: true,
            },
        ]
    );
    assert_eq!(
        export("OUTPUT EXPORT NAME=* /HTML DOCUMENTFILE='a.htm'.").name,
        None
    );
}

#[test]
fn output_export_errors() {
    for (source, message) in [
        (
            "OUTPUT EXPORT /PDF DOCUMENTFILE='a.pdf'.",
            "cannot write PDF files (/PDF)",
        ),
        (
            "OUTPUT EXPORT /DOC DOCUMENTFILE='a.doc'.",
            "cannot write Word files",
        ),
        (
            "OUTPUT EXPORT /XLS DOCUMENTFILE='a.xls'.",
            "/XLSX, /HTML, /TEXT or /CSV",
        ),
        ("OUTPUT EXPORT /PNG IMAGEROOT='a'.", "draws no charts"),
        (
            "OUTPUT EXPORT /HTML NOTESCAPTIONS=NO.",
            "/HTML needs DOCUMENTFILE",
        ),
        (
            "OUTPUT EXPORT /XLSX DOCUMENTFILE='a' OPERATION=MODIFYSHEET.",
            "needs SHEET",
        ),
        (
            "OUTPUT EXPORT /XLSX DOCUMENTFILE='a' SHEET='a/b'.",
            "sheet name",
        ),
        (
            "OUTPUT EXPORT /XLSX DOCUMENTFILE='a' LOCATION=STARTCELL('2B').",
            "no cell",
        ),
        (
            "OUTPUT EXPORT /HTML DOCUMENTFILE='a' /HTML DOCUMENTFILE='b'.",
            "given twice",
        ),
        (
            "OUTPUT EXPORT /HTML DOCUMENTFILE='a' DOCUMENTFILE='b'.",
            "given twice",
        ),
        // keywords are spelled in full
        (
            "OUTPUT EXPORT /HTML DOCFILE='a'.",
            "unexpected word DOCFILE",
        ),
        (
            "OUTPUT EXPORT /TEXT DOCUMENTFILE='a' ROWBORDER='--'.",
            "one character",
        ),
    ] {
        let error = err(source);
        assert!(error.contains(message), "{source}: {error}");
    }
}

#[test]
fn output_save_is_refused_clearly() {
    let error = err("OUTPUT SAVE OUTFILE='a.spv'.");
    assert!(
        error.contains("no output document file of its own"),
        "{error}"
    );
    assert!(error.contains("OUTPUT EXPORT"), "{error}");
}

#[test]
fn report_reads_the_manual_examples() {
    let r = report(
        "REPORT FORMAT=LIST\n\
         /VARIABLES=PRODUCT (LABEL) ' ' 'Retail' 'Products'\n\
                  SALES 'Annual' 'Sales' '1981'\n\
         /BREAK=DEPT 'Department' (LABEL)\n\
         /SUMMARY=VALIDN (PRODUCT) MEAN (SALES) 'No.Sold,Mean Sales'.",
    );
    assert_eq!(r.format.list, Some(0));
    assert_eq!(r.columns.len(), 2);
    assert_eq!(
        r.columns[0].variables,
        [(VarItem::Name("PRODUCT".into()), ColumnContents::Label)]
    );
    assert_eq!(
        r.columns[0].heading,
        Some(vec![" ".into(), "Retail".into(), "Products".into()])
    );
    let b = &r.breaks[0];
    assert_eq!(b.heading, Some(vec!["Department".into()]));
    assert_eq!(b.label, Some(true));
    let ReportSummary::Line { items, title, .. } = &b.summaries[0] else {
        panic!("{:?}", b.summaries);
    };
    assert_eq!(title.as_deref(), Some("No.Sold,Mean Sales"));
    assert_eq!(items.len(), 2);

    let r = report(
        [removed]
    );
    assert!(r.format.automatic);
    assert_eq!(r.format.brkspace, Some(-1));
    assert_eq!(r.format.margins, (Some(1), Some(70)));
    assert_eq!(r.columns[0].variables[0].1, ColumnContents::Dummy);
    assert_eq!(r.columns[1].width, Some(15));
    assert_eq!(r.columns[1].offset, Some(ReportOffset::Columns(2)));
    assert!(r.breaks[0].total);
    assert_eq!(r.breaks.len(), 2);

    let r = report(
        [removed]
    );
    let lines = &r.breaks[0].summaries;
    assert_eq!(lines.len(), 3);
    let ReportSummary::Line { items, skip, .. } = &lines[2] else {
        panic!()
    };
    assert_eq!(*skip, 1);
    let SummaryItem::Composite {
        function,
        args,
        column,
    } = &items[0]
    else {
        panic!("{items:?}")
    };
    assert_eq!(*function, Composite::Divide);
    assert_eq!(
        args[0],
        CompositeArg::Aggregate(ReportFunction::Mean, "WAGE".into())
    );
    let (name, format) = column.as_ref().unwrap();
    assert_eq!(name, "SPACE1");
    assert_eq!(format.kind, Some(FormatType::Comma));
    assert_eq!(format.decimals, Some(2));

    let r = report(
        "REPORT FORMAT=AUTOMATIC LIST /VARIABLES=XVAR YVAR ZVAR\n\
         /BREAK=BVAR /SUMMARY=SUM /SUMMARY=MEAN (XVAR YVAR ZVAR) /SUMMARY=VALIDN(XVAR)\n\
         /BREAK=AVAR /SUMMARY=PREVIOUS.",
    );
    assert_eq!(r.breaks[1].summaries, [ReportSummary::Previous(None)]);

    let r = report(
        "REPORT /STRING=JOB1(AVAR NVAR) JOB2(AVAR(2) NVAR(3)) JOB3(AVAR(2) NVAR(BLANK) (4))\n\
         SOCSEC(S1 '-' S2 '-' S3)\n\
         /VARIABLES=JOB1 /MISSING=LIST(XVAR,YVAR,ZVAR 2)\n\
         /TITLE=LEFT 'Personnel Report' 'Prepared on )DATE' RIGHT 'Page: )PAGE'\n\
         /BREAK=(TOTAL) /SUMMARY=PGT(5)(JOB1) MEDIAN(1,10) 'x' /SUMMARY=FREQUENCY(1,3).",
    );
    assert_eq!(r.strings.len(), 4);
    assert_eq!(
        r.strings[2].parts[1],
        StringPart::Variable {
            name: "NVAR".into(),
            width: Some(4),
            blank: true
        }
    );
    assert_eq!(
        r.missing,
        ReportMissing::List(
            vec![
                VarItem::Name("XVAR".into()),
                VarItem::Name("YVAR".into()),
                VarItem::Name("ZVAR".into())
            ],
            2
        )
    );
    assert_eq!(r.titles[0].0, ReportAlign::Left);
    assert_eq!(
        r.titles[1],
        (ReportAlign::Right, vec!["Page: )PAGE".to_string()])
    );
    assert!(r.breaks[0].variables.is_empty());
}

#[test]
fn report_errors() {
    for (source, message) in [
        ("REPORT FORMAT=LIST.", "needs VARIABLES"),
        ("REPORT /VARIABLES=a.", "needs BREAK and SUMMARY"),
        (
            "REPORT /VARIABLES=a /FORMAT=LIST.",
            "FORMAT must come first",
        ),
        (
            "REPORT /SUMMARY=MEAN /VARIABLES=a.",
            "SUMMARY must follow a BREAK",
        ),
        (
            "REPORT /VARIABLES=a /BREAK=b /MISSING=NONE /SUMMARY=MEAN.",
            "MISSING must come",
        ),
        (
            "REPORT /VARIABLES=a /BREAK=b /SUMMARY=PREVIOUS.",
            "earlier BREAK",
        ),
        (
            "REPORT /VARIABLES=a /BREAK=b /SUMMARY=FREQUENCY(1,2) MEAN.",
            "stand alone",
        ),
        (
            "REPORT /VARIABLES=a /BREAK=b /SUMMARY=MEDIAN(5,1).",
            "minimum is above",
        ),
        (
            "REPORT /VARIABLES=a /BREAK=b /SUMMARY=PCT(SUM(a)).",
            "PCT takes two",
        ),
        (
            "REPORT /VARIABLES=a /BREAK=b /BREAK=c (TOTAL) /SUMMARY=MEAN.",
            "first BREAK",
        ),
        ("REPORT FORMAT=LIST BRKSPACE(-2) /VARIABLES=a.", "-1"),
        (
            "REPORT /VARIABLES=a /VARIABLES=b /BREAK=c /SUMMARY=SUM.",
            "only once",
        ),
    ] {
        let error = err(source);
        assert!(error.contains(message), "{source}: {error}");
    }
}

#[test]
fn finish_ends_the_syntax() {
    let (parsed, errors) = parse("FINISH.\nNOT A COMMAND 'unclosed.\n");
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].command, Command::Finish);
    let (parsed, errors) = parse("TITLE a.\nFIN.\nFREQUENCIES x.\n");
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(parsed.len(), 2);
    // with a line range only a FINISH inside it counts
    let source = "FINISH.\nTITLE a.\nTITLE b.\n";
    let all = split_commands_with(source, None, &mut MacroState::default(), Some((2, 3))).unwrap();
    assert_eq!(all.len(), 3);
}

#[test]
fn stray_ends_of_blocks_say_where_they_belong() {
    let error = err("END DATA.");
    assert!(error.contains("END DATA without BEGIN DATA"), "{error}");
    let error = err("END REPEAT.");
    assert!(error.contains("END REPEAT without DO REPEAT"), "{error}");
    let error = err("END MATRIX.");
    assert!(error.contains("END MATRIX without MATRIX"), "{error}");
    assert!(!unsupported_commands().contains(&"END DATA"));
    assert!(!unsupported_commands().contains(&"FINISH"));
    assert!(help(Some("END DATA")).unwrap().contains("BEGIN DATA"));
    assert!(help(Some("output save")).unwrap().contains("OUTPUT EXPORT"));
    assert!(help(Some("GRAPH")).unwrap().contains("not supported yet"));
}
