//! REPORT on synthetic data laid out as the manual's examples. The expected numbers are
//! worked out by hand: Toys sell 10 and 20 (sum 30, mean 15), Books 30 and 50 with one sale
//! missing (sum 80, mean 40), all four valid sales sum to 110.

use statscraft_engine::session::Session;
use statscraft_engine::{Cell, Table};

const DATA: &str = "DATA LIST FREE /dept product sales.
BEGIN DATA
1 1 10
1 2 20
2 1 30
2 3 50
2 2 .
END DATA.
VALUE LABELS dept 1 'Toys' 2 'Books' /product 1 'Ball' 2 'Kite' 3 'Atlas'.
";

fn run(source: &str) -> (Vec<Table>, Vec<String>) {
    let mut session = Session::default();
    let result = session
        .run(&format!("{DATA}{source}"), None)
        .unwrap_or_else(|e| panic!("{source}\n{e:?}"));
    let tables = result.commands.iter().flat_map(|c| c.tables.clone());
    let warnings = result.commands.iter().flat_map(|c| c.warnings.clone());
    (tables.collect(), warnings.collect())
}

fn report(source: &str) -> Table {
    let (tables, _) = run(source);
    tables
        .into_iter()
        .rfind(|t| t.title != "Echo")
        .expect("a report")
}

fn fails(source: &str) -> String {
    let errors = Session::default()
        .run(&format!("{DATA}{source}"), None)
        .unwrap_err();
    errors
        .iter()
        .map(|e| e.text())
        .collect::<Vec<_>>()
        .join(" | ")
}

/// The rows as text, empty cells as "".
fn rows(table: &Table) -> Vec<Vec<String>> {
    table
        .rows
        .iter()
        .map(|r| {
            r.iter()
                .map(|c| match c {
                    Cell::Text(t) => t.clone(),
                    Cell::Number(n) => n.to_string(),
                    Cell::Empty => String::new(),
                })
                .collect()
        })
        .collect()
}

fn expect(table: &Table, expected: &[&[&str]]) {
    let expected: Vec<Vec<String>> = expected
        .iter()
        .map(|r| r.iter().map(|c| c.to_string()).collect())
        .collect();
    assert_eq!(rows(table), expected);
}

#[test]
fn a_listing_with_break_groups() {
    let table = report(
        "REPORT FORMAT=LIST
  /VARIABLES=product (LABEL) ' ' 'Retail' 'Products' sales 'Annual' 'Sales'
  /BREAK=dept 'Department' (LABEL)
  /SUMMARY=VALIDN (product) MEAN (sales) 'No.Sold,Mean Sales'.",
    );
    assert_eq!(table.title, "Report");
    assert_eq!(
        table.columns,
        ["Department", "Retail Products", "Annual Sales"]
    );
    assert_eq!(table.row_headers, 1);
    expect(
        &table,
        &[
            &["Toys", "", ""],
            &["", "Ball", "10.00"],
            &["", "Kite", "20.00"],
            &["No.Sold,Mean Sales", "2", "15.00"],
            &["Books", "", ""],
            &["", "Ball", "30.00"],
            &["", "Atlas", "50.00"],
            &["", "Kite", "."],
            &["No.Sold,Mean Sales", "3", "40.00"],
        ],
    );
}

#[test]
fn automatic_summaries_share_the_break_line_and_totals_follow() {
    let table = report(
        "REPORT FORMAT=AUTOMATIC /VARIABLES=sales /BREAK=dept (TOTAL)
  /SUMMARY=SUM /SUMMARY=MEAN /SUMMARY=MIN 'Lowest'.",
    );
    assert_eq!(table.columns, ["dept", "sales"]);
    expect(
        &table,
        &[
            &["Toys", "30.00"],
            &["Mean", "15.00"],
            &["Lowest", "10.00"],
            &["Books", "80.00"],
            &["Mean", "40.00"],
            &["Lowest", "30.00"],
            &["Grand Total", ""],
            &["Sum", "110.00"],
        ],
    );
    // values instead of labels, and the head on a line of its own
    let table = report(
        "REPORT FORMAT=AUTOMATIC BRKSPACE(0) /VARIABLES=sales /BREAK=dept (VALUE) /SUMMARY=SUM.",
    );
    expect(
        &table,
        &[
            &["1.00", ""],
            &["Sum", "30.00"],
            &["2.00", ""],
            &["Sum", "80.00"],
        ],
    );
}

#[test]
fn nested_breaks_and_previous() {
    let table = report(
        "REPORT FORMAT=AUTOMATIC /VARIABLES=sales
  /BREAK=dept /SUMMARY=SUM
  /BREAK=product (VALUE) /SUMMARY=PREVIOUS.",
    );
    assert_eq!(table.columns, ["dept", "product", "sales"]);
    // a group of product closes before its dept; Books has Ball, Atlas, Kite
    expect(
        &table,
        &[
            &["Toys", "1.00", "10.00"],
            &["", "2.00", "20.00"],
            &["Sum", "", "30.00"],
            &["Books", "1.00", "30.00"],
            &["", "3.00", "50.00"],
            &["", "2.00", "."],
            &["Sum", "", "80.00"],
        ],
    );
}

#[test]
fn statistics_and_composites() {
    let table = report(
        "REPORT /VARIABLES=sales spare (DUMMY) /BREAK=(TOTAL)
  /SUMMARY=PGT(25)(sales) PCT(SUM(sales) 220)(spare)
  /SUMMARY=VARIANCE(sales (PLAIN)(1)) DIVIDE(MAX(sales) MIN(sales) 10)(spare)
  /SUMMARY=MEDIAN(0,100) (sales) 'Median'
  /SUMMARY=STDDEV ADD(VALIDN(sales) 1 2) (spare (F)(1)).",
    );
    assert_eq!(table.columns, ["", "sales", "spare"]);
    // variance of 10 20 30 50: mean 27.5, squares 306.25 56.25 6.25 506.25 = 875, / 3
    expect(
        &table,
        &[
            &[">25", "50.0%", "50.00%"],
            &["Variance", "291.7", "50"],
            &["Median", "25.00", ""],
            &["StdDev", "17.08", "7.0"],
        ],
    );
}

#[test]
fn frequencies_mode_and_weights() {
    let table = report(
        "REPORT /VARIABLES=product /BREAK=(TOTAL) /SUMMARY=MODE(1,3)
  /SUMMARY=FREQUENCY(1,3) /SUMMARY=PERCENT(1,2) 'In range'.",
    );
    expect(
        &table,
        &[
            &["Mode", "1.00"],
            &["1", "2"],
            &["2", "2"],
            &["3", "1"],
            &["Total", "5"],
            &["1", "40.0"],
            &["2", "40.0"],
            &["In range", "80.0"],
        ],
    );
    // a weight counts each case that often
    let table = report(
        "COMPUTE w = dept.\nWEIGHT BY w.\nREPORT /VARIABLES=sales /BREAK=(TOTAL) /SUMMARY=VALIDN /SUMMARY=MEAN.",
    );
    // weights 1 1 2 2: (10 + 20 + 60 + 100) / 6
    expect(&table, &[&["N", "6"], &["Mean", "31.67"]]);
    let e = fails("REPORT /VARIABLES=sales /BREAK=(TOTAL) /SUMMARY=VALIDN MEAN.");
    assert!(e.contains("two results of one SUMMARY line"), "{e}");
}

#[test]
fn missing_values() {
    // 20 is user-missing: listed, but not counted; NONE counts it; LIST drops the cases
    let base = "MISSING VALUES sales (20).\n";
    let table = report(&format!(
        "{base}REPORT FORMAT=LIST /VARIABLES=sales /BREAK=(TOTAL) /SUMMARY=SUM."
    ));
    expect(
        &table,
        &[
            &["", "10.00"],
            &["", "20.00"],
            &["", "30.00"],
            &["", "50.00"],
            &["", "."],
            &["Sum", "90.00"],
        ],
    );
    let table = report(&format!(
        "{base}REPORT /VARIABLES=sales /MISSING=NONE /BREAK=(TOTAL) /SUMMARY=SUM."
    ));
    expect(&table, &[&["Sum", "110.00"]]);
    let table = report(&format!(
        "{base}REPORT FORMAT=LIST MISSING '*' /VARIABLES=sales product /MISSING=LIST\n\
         /BREAK=(TOTAL) /SUMMARY=VALIDN(product)."
    ));
    expect(
        &table,
        &[
            &["", "10.00", "1.00"],
            &["", "30.00", "1.00"],
            &["", "50.00", "3.00"],
            &["N", "", "3"],
        ],
    );
}

#[test]
fn strings_stacks_and_titles() {
    let table = report(
        "TITLE 'Sales 2026'.\n\
         REPORT FORMAT=LIST /STRING=code(dept(2) '-' product(BLANK)(3))\n\
         /VARIABLES=code + sales\n\
         /TITLE=LEFT 'By code' RIGHT 'Page )PAGE' /FOOTNOTE='First )dept'.",
    );
    assert_eq!(table.title, "By code    Page     1");
    assert_eq!(table.caption.as_deref(), Some("First Toys"));
    assert_eq!(table.columns, ["code"]);
    assert_eq!(
        rows(&table)[..4],
        [["01-  1"], ["10.00"], ["01-  2"], ["20.00"]]
    );
    // the TITLE command's text is the default title
    let table = report("TITLE 'Sales 2026'.\nREPORT FORMAT=LIST /VARIABLES=sales.");
    assert_eq!(table.title, "Sales 2026");
    let e = fails("REPORT /VARIABLES=dept + sales /BREAK=(TOTAL) /SUMMARY=SUM.");
    assert!(e.contains("only with FORMAT=LIST"), "{e}");
    let e = fails("REPORT /VARIABLES=product /STRING=x(dept) /BREAK=(TOTAL) /SUMMARY=SUM.");
    assert!(e.contains("STRING must come before VARIABLES"), "{e}");
    let e = fails("REPORT /STRING=s(product) /VARIABLES=s /BREAK=(TOTAL) /SUMMARY=MEAN.");
    assert!(e.contains("only VALIDN works on a string variable"), "{e}");
    let e = fails("REPORT /VARIABLES=sales dept (DUMMY) /BREAK=(TOTAL) /SUMMARY=SUM(sales).");
    assert!(e.contains("cannot be a DUMMY column"), "{e}");
}

#[test]
fn outfile_writes_the_report_as_text() {
    let dir = std::env::temp_dir().join(format!("statscraft-report-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let job = format!(
        "CD '{}'.\n\
         REPORT FORMAT=LIST /OUTFILE='r.txt' /VARIABLES=product (LABEL) sales 'Annual' 'Sales'\n\
           /TITLE='Sales' /BREAK=dept (LABEL) /SUMMARY=MEAN(sales).\n\
         REPORT FORMAT=AUTOMATIC COLSPACE(2) /OUTFILE='r.txt' /VARIABLES=sales (10)\n\
           /BREAK=dept (TOTAL) /SUMMARY=SUM.",
        dir.display()
    );
    let (tables, _) = run(&job);
    assert!(tables.iter().all(|t| t.title != "Report"));
    let text = std::fs::read_to_string(dir.join("r.txt")).unwrap();
    let lines = [
        "             Sales",
        "",
        "dept        product       Annual",
        "                           Sales",
        "",
        "Toys",
        "",
        "            Ball           10.00",
        "            Kite           20.00",
        "",
        "Mean                       15.00",
        "",
        "Books",
        "",
        "            Ball           30.00",
        "            Atlas          50.00",
        "            Kite               .",
        "",
        "Mean                       40.00",
        // the second report, AUTOMATIC: centred, underscored headings, values on the break line
        "dept     sales",
        "-----  ----------",
        "",
        "Toys     30.00",
        "",
        "Books    80.00",
        "",
        "Grand Total",
        "Sum      110.00",
    ];
    let expected = lines.join("\n") + "\n";
    assert_eq!(text, expected, "\n{text}");
    // a later session starts the file again
    run(&job);
    assert_eq!(
        std::fs::read_to_string(dir.join("r.txt")).unwrap(),
        expected
    );
    std::fs::remove_dir_all(dir).unwrap();
}
