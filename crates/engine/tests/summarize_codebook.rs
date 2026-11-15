//! SUMMARIZE and CODEBOOK on a small invented bakery file. Expected numbers are worked out
//! by hand: the North shop sold 10, 20 and 30 (mean 20), the South shop 40 and 50 with one
//! sale missing (mean 45); all five sales sum to 150 (mean 30). Hours open are 4, 6 and
//! missing in the North, 8, 9 and 7 in the South.

use statscraft_engine::session::Session;
use statscraft_engine::{Cell, Table};

const DATA: &str = "DATA LIST FREE /shop kind sales hours.
BEGIN DATA
1 1 10 4
1 1 20 6
1 2 30 .
2 1 40 8
2 2 50 9
2 2 . 7
END DATA.
VALUE LABELS shop 1 'North' 2 'South' /kind 1 'Bread' 2 'Cake'.
VARIABLE LEVEL sales hours (SCALE) /shop kind (NOMINAL).
";

fn run(source: &str) -> (Vec<Table>, Vec<String>) {
    let mut session = Session::default();
    let result = session
        .run(&format!("{DATA}{source}"), None)
        .unwrap_or_else(|e| panic!("{source}\n{e:?}"));
    let tables = result.commands.iter().flat_map(|c| c.tables.clone());
    let warnings = result.commands.iter().flat_map(|c| c.warnings.clone());
    (
        tables.filter(|t| t.title != "Echo").collect(),
        warnings.collect(),
    )
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

fn cell(c: &Cell) -> String {
    match c {
        Cell::Text(t) => t.clone(),
        Cell::Number(n) => format!("{}", (n * 1e9).round() / 1e9),
        Cell::Empty => String::new(),
    }
}

fn rows(table: &Table) -> Vec<Vec<String>> {
    table
        .rows
        .iter()
        .map(|r| r.iter().map(cell).collect())
        .collect()
}

fn expect(table: &Table, expected: &[&[&str]]) {
    let expected: Vec<Vec<String>> = expected
        .iter()
        .map(|r| r.iter().map(|c| c.to_string()).collect())
        .collect();
    assert_eq!(rows(table), expected, "{}", table.title);
}

fn titled<'a>(tables: &'a [Table], title: &str) -> &'a Table {
    tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"))
}

#[test]
fn summary_report_puts_dependents_side_by_side() {
    let (tables, _) = run("SUMMARIZE TABLES=sales hours BY shop /CELLS=COUNT MEAN SUM.");
    let summary = titled(&tables, "Case Processing Summary");
    expect(
        summary,
        &[
            &[
                "sales * shop",
                "5",
                "83.333333333",
                "1",
                "16.666666667",
                "6",
                "100",
            ],
            &[
                "hours * shop",
                "5",
                "83.333333333",
                "1",
                "16.666666667",
                "6",
                "100",
            ],
        ],
    );
    let report = titled(&tables, "Case Summaries");
    assert_eq!(report.columns, ["shop", "", "sales", "hours"]);
    assert_eq!(report.row_headers, 2);
    expect(
        report,
        &[
            &["North", "N", "3", "2"],
            &["North", "Mean", "20", "5"],
            &["North", "Sum", "60", "10"],
            &["South", "N", "2", "3"],
            &["South", "Mean", "45", "8"],
            &["South", "Sum", "90", "24"],
            &["Total", "N", "5", "5"],
            &["Total", "Mean", "30", "6.8"],
            &["Total", "Sum", "150", "34"],
        ],
    );
}

#[test]
fn default_cells_are_mean_count_and_standard_deviation() {
    let (tables, _) = run("SUMM sales.");
    let report = titled(&tables, "Case Summaries");
    // 10 20 30 40 50: deviations 20 10 0 10 20, squares sum 1000, over 4
    expect(
        report,
        &[
            &["Mean", "30"],
            &["N", "5"],
            &["Std. Deviation", "15.811388301"],
        ],
    );
}

#[test]
fn missing_table_drops_cases_missing_on_any_dependent() {
    let (tables, _) =
        run("SUMMARIZE sales hours BY shop /MISSING=TABLE /CELLS=MEAN /FORMAT=NOTOTAL.");
    expect(
        titled(&tables, "Case Summaries"),
        &[
            &["North", "Mean", "15", "5"],
            &["South", "Mean", "45", "8.5"],
            &["", "Mean", "30", "6.75"],
        ],
    );
}

#[test]
fn listing_shows_cases_before_the_group_totals() {
    let (tables, _) = run("SUMMARIZE sales BY shop /FORMAT=LIST /CELLS=COUNT.");
    let report = titled(&tables, "Case Summaries");
    assert_eq!(report.row_headers, 3);
    expect(
        report,
        &[
            &["North", "1", "", "10"],
            &["North", "2", "", "20"],
            &["North", "3", "", "30"],
            &["North", "Total", "N", "3"],
            &["South", "4", "", "40"],
            &["South", "5", "", "50"],
            &["South", "Total", "N", "2"],
            &["Total", "", "N", "5"],
        ],
    );
}

#[test]
fn valid_listing_limit_and_missing_text() {
    let (tables, _) =
        run("SUMMARIZE sales hours /FORMAT=VALIDLIST NOCASENUM /CELLS=NONE /TITLE='Open shops'.");
    let report = titled(&tables, "Open shops");
    expect(
        report,
        &[
            &["", "", "10", "4"],
            &["", "", "20", "6"],
            &["", "", "40", "8"],
            &["", "", "50", "9"],
        ],
    );
    let (tables, _) = run(
        "SUMMARIZE sales hours /FORMAT=LIST LIMIT=4 MISSING='n/a' /CELLS=COUNT /FOOTNOTE='Shops'.",
    );
    let report = titled(&tables, "Case Summaries");
    expect(
        report,
        &[
            &["1", "", "10", "4"],
            &["2", "", "20", "6"],
            &["3", "", "30", "n/a"],
            &["4", "", "40", "8"],
            &["Total", "N", "4", "3"],
        ],
    );
    assert_eq!(
        report.caption.as_deref(),
        Some("Shops\nLimited to the first 4 cases.")
    );
}

#[test]
fn percentages_weights_and_analysis_of_variance() {
    let (tables, _) = run("SUMMARIZE sales BY shop /CELLS=NPCT SPCT NPCT(shop) /STATISTICS=ANOVA.");
    expect(
        titled(&tables, "Case Summaries"),
        &[
            &["North", "% of Total N", "60"],
            &["North", "% of Total Sum", "40"],
            &["North", "% of Total N in shop", "100"],
            &["South", "% of Total N", "40"],
            &["South", "% of Total Sum", "60"],
            &["South", "% of Total N in shop", "100"],
            &["Total", "% of Total N", "100"],
            &["Total", "% of Total Sum", "100"],
            &["Total", "% of Total N in shop", "100"],
        ],
    );
    // between: 3 (20 - 30)^2 + 2 (45 - 30)^2 = 750; within: 200 + 50 = 250 on 3 df
    let anova = titled(&tables, "ANOVA Table: sales by shop");
    assert_eq!(cell(&anova.rows[0][2]), "750");
    assert_eq!(cell(&anova.rows[0][5]), "9");
    // weighted by kind: North 10 + 20 + 2 * 30 over 4
    let (tables, _) = run("WEIGHT BY kind.\nSUMMARIZE sales BY shop /CELLS=COUNT MEAN.");
    let report = titled(&tables, "Case Summaries");
    assert_eq!(rows(report)[0], ["North", "N", "4"]);
    assert_eq!(rows(report)[1], ["North", "Mean", "22.5"]);
}

#[test]
fn summarize_rules_on_lists_and_repeats() {
    assert!(fails("SUMMARIZE sales BY shop BY shop.").contains("two BY"));
    assert!(fails("SUMMARIZE sales /hours /FORMAT=LIST.").contains("one table list"));
    let (_, warnings) = run("SUMMARIZE sales /CELLS=MEAN /CELLS=SUM.");
    assert!(warnings.iter().any(|w| w.contains("CELLS")), "{warnings:?}");
}

#[test]
fn codebook_entries_for_nominal_and_scale_variables() {
    let (tables, _) = run("CODEBOOK shop sales /VARINFO=POSITION TYPE MEASURE ROLE MISSING.");
    assert_eq!(tables.len(), 2);
    let shop = &tables[0];
    assert_eq!(shop.title, "shop");
    assert_eq!(shop.columns, ["", "", "Value", "Count", "Percent"]);
    expect(
        shop,
        &[
            &["Standard Attributes", "Position", "1", "", ""],
            &["", "Type", "Numeric", "", ""],
            &["", "Measurement", "Nominal", "", ""],
            &["", "Role", "Input", "", ""],
            &["", "Missing Values", "", "", ""],
            &["Valid Values", "1", "North", "3", "50"],
            &["", "2", "South", "3", "50"],
        ],
    );
    // quartiles at (5 + 1) p: positions 1.5, 3 and 4.5
    expect(
        &tables[1],
        &[
            &["Standard Attributes", "Position", "3", "", ""],
            &["", "Type", "Numeric", "", ""],
            &["", "Measurement", "Scale", "", ""],
            &["", "Role", "Input", "", ""],
            &["", "Missing Values", "", "", ""],
            &["Central Tendency and Dispersion", "Mean", "30", "", ""],
            &["", "Standard Deviation", "15.811388301", "", ""],
            &["", "Percentile 25", "15", "", ""],
            &["", "Percentile 50", "30", "", ""],
            &["", "Percentile 75", "45", "", ""],
            &["Missing Values", "System", "", "1", "16.666666667"],
        ],
    );
}

#[test]
fn codebook_levels_missing_values_and_maxcats() {
    let (tables, _) =
        run("MISSING VALUES hours (9).\nCODEBOOK hours [N] /VARINFO=LABEL /STATISTICS=COUNT.");
    expect(
        &tables[0],
        &[
            &["Standard Attributes", "Label", "", ""],
            &["Valid Values", "4", "", "1"],
            &["", "6", "", "1"],
            &["", "7", "", "1"],
            &["", "8", "", "1"],
            &["Missing Values", "9", "", "1"],
            &["", "System", "", "1"],
        ],
    );
    let (tables, _) = run("CODEBOOK hours [N] /VARINFO=LABEL /OPTIONS MAXCATS=3.");
    assert_eq!(tables[0].rows.len(), 1);
    assert!(tables[0].caption.as_deref().unwrap().contains("MAXCATS"));
}

#[test]
fn codebook_order_sets_and_file_information() {
    let (tables, _) = run("CODEBOOK /VARINFO=LABEL /STATISTICS=NONE /OPTIONS VARORDER=ALPHA.");
    let titles: Vec<&str> = tables.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(titles, ["hours", "kind", "sales", "shop"]);
    let (tables, _) = run(
        "CODEBOOK sales shop /VARINFO=LABEL /STATISTICS=NONE /OPTIONS VARORDER=MEASURE SORT=DESCENDING.",
    );
    let titles: Vec<&str> = tables.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(titles, ["sales", "shop"]);
    let (tables, _) = run("COMPUTE bread = kind = 1.
COMPUTE big = sales > 25.
MRSETS /MDGROUP NAME=$picks LABEL='Picks' VARIABLES=bread big VALUE=1
  /MCGROUP NAME=$places VARIABLES=shop kind.
CODEBOOK $ALL /VARINFO=LABEL /STATISTICS=COUNT PERCENT.");
    expect(
        &tables[0],
        &[
            &["Standard Attributes", "Label", "Picks", "", ""],
            &["Valid Values", "bread", "", "3", "50"],
            &["", "big", "", "3", "50"],
        ],
    );
    // each case once per value it gives in any variable of the set
    expect(
        &tables[1],
        &[
            &["Standard Attributes", "Label", "", "", ""],
            &["Valid Values", "1", "North", "4", "66.666666667"],
            &["", "2", "South", "4", "66.666666667"],
        ],
    );
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("codebook_shops.sav");
    let (tables, _) = run(&format!(
        "FILE LABEL 'Bakery'.
SAVE OUTFILE='{}'.
COMPUTE open = shop = 1.
FILTER BY open.
SPLIT FILE BY kind.
CODEBOOK shop /FILEINFO=NAME CASECOUNT LABEL WEIGHT /VARINFO=LABEL /STATISTICS=COUNT PERCENT.",
        path.display()
    ));
    expect(
        &tables[0],
        &[
            &["File Name", "", "codebook_shops.sav"],
            &["Number of Cases", "", "6"],
            &["Label", "", "Bakery"],
            &["Weight", "", "None"],
        ],
    );
    // the filter keeps the North shop; the split is ignored
    expect(
        &tables[1],
        &[
            &["Standard Attributes", "Label", "", "", ""],
            &["Valid Values", "1", "North", "3", "100"],
            &["", "2", "South", "0", "0"],
        ],
    );
}

#[test]
fn codebook_rules_on_its_list() {
    assert!(fails("CODEBOOK shop shop.").contains("more than once"));
    assert!(fails("CODEBOOK ALL shop.").contains("more than once"));
    assert!(fails("STRING code (A2).\nCODEBOOK code [S].").contains("string"));
    assert!(fails("CODEBOOK $none.").contains("unknown multiple response set"));
    assert!(fails("CODEBOOK /STATISTICS=NONE COUNT.").contains("alone"));
    assert!(fails("CODEBOOK /VARINFO=LABEL /VARINFO=TYPE.").contains("once"));
}
