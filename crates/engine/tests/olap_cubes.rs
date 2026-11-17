//! OLAP CUBES on a small invented sales file: four stores, two years, revenue and cost, a
//! weight. Revenue 999 is user-missing; store 4 has no valid revenue at all.
//!
//! Where the expected numbers come from: the cell statistics that MEANS also has were checked
//! against GNU PSPP 2.0.0's MEANS run as a black box on the same data (weighted and
//! unweighted); the sums, percentages and differences are worked out by hand in comments.

use statscraft_engine::session::Session;
use statscraft_engine::{Cell, Table};

const DATA: &str = "DATA LIST FREE /store year revenue cost w.
BEGIN DATA
1 2024 10 4 1
1 2024 14 6 2
1 2025 12 5 1
1 2025 20 . 1
2 2024 8 3 1
2 2024 999 2 1
2 2025 16 7 3
2 2025 18 9 1
3 2024 5 2 1
3 2025 9 1 2
4 2025 . 6 1
END DATA.
MISSING VALUES revenue (999).
VALUE LABELS store 1 'North' 2 'South' 3 'East' 4 'West'.
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

/// Numbers rounded to 4 decimals.
fn cell(c: &Cell) -> String {
    match c {
        Cell::Text(t) => t.clone(),
        Cell::Number(n) => format!("{}", (n * 1e4).round() / 1e4),
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
fn default_cells_with_percentages_of_the_totals() {
    let (tables, _) = run("OLAP CUBES revenue BY store.");
    // revenue valid in 9 of 11 cases (999 user-missing, one system-missing)
    expect(
        titled(&tables, "Case Processing Summary"),
        &[&[
            "revenue * store",
            "9",
            "81.8182",
            "2",
            "18.1818",
            "11",
            "100",
        ]],
    );
    let report = titled(&tables, "OLAP Cubes");
    assert_eq!(
        report.columns,
        [
            "store",
            "",
            "Sum",
            "N",
            "Mean",
            "Std. Deviation",
            "% of Total Sum",
            "% of Total N"
        ]
    );
    // North 10 14 12 20: sum 56, sd sqrt(56/3); South 8 16 18: sum 42, sd sqrt(28); East 5 9:
    // sum 14, sd sqrt(8); all: sum 112 of 9, sd sqrt(196.2222/8). Percentages of 112 and of 9.
    expect(
        report,
        &[
            &[
                "North", "revenue", "56", "4", "14", "4.3205", "50", "44.4444",
            ],
            &[
                "South", "revenue", "42", "3", "14", "5.2915", "37.5", "33.3333",
            ],
            &[
                "East", "revenue", "14", "2", "7", "2.8284", "12.5", "22.2222",
            ],
            &[
                "Total", "revenue", "112", "9", "12.4444", "4.9526", "100", "100",
            ],
        ],
    );
}

#[test]
fn every_cell_statistic_agrees_with_means_unweighted() {
    let (tables, _) = run("OLAP CUBES revenue cost BY store BY year
        /CELLS=MEAN COUNT STDDEV SUM SEMEAN MIN MAX RANGE VARIANCE KURT SEKURT SKEW SESKEW
        FIRST LAST HARMONIC GEOMETRIC.");
    let report = titled(&tables, "OLAP Cubes");
    let row = |store: &str, year: &str, variable: &str| {
        rows(report)
            .into_iter()
            .find(|r| r[0] == store && r[1] == year && r[2] == variable)
            .unwrap_or_else(|| panic!("no row {store} {year} {variable}"))[3..]
            .to_vec()
    };
    // PSPP MEANS, all stores and years: revenue mean 12.444444, sd 4.952553, se 1.650851,
    // variance 24.53, kurtosis -1.00, se 1.40, skewness .13, se .72, harmonic 10.48, geometric
    // 11.49
    let total = row("Total", "Total", "revenue");
    assert_eq!(
        total[..9],
        [
            "12.4444", "9", "4.9526", "112", "1.6509", "5", "20", "15", "24.5278"
        ]
    );
    let close = |text: &str, want: f64, digits: i32| {
        let got: f64 = text.parse().unwrap();
        assert!(
            (got - want).abs() < 0.5 * 10f64.powi(-digits),
            "{got} vs {want}"
        );
    };
    close(&total[9], -1.00, 2);
    close(&total[10], 1.40, 2);
    close(&total[11], 0.13, 2);
    close(&total[12], 0.72, 2);
    assert_eq!(total[13..15], ["10", "9"]);
    close(&total[15], 10.48, 2);
    close(&total[16], 11.49, 2);
    // North in 2024, cost: PSPP mean 5, sd 1.41, harmonic 4.80, geometric 4.90
    let north = row("North", "2024", "cost");
    assert_eq!(north[..4], ["5", "2", "1.4142", "10"]);
    close(&north[15], 4.80, 2);
    close(&north[16], 4.90, 2);
    // store 4 has cost but no valid revenue: its revenue row is there with N 0
    let west = row("West", "2025", "revenue");
    assert_eq!(west[1], "0");
    assert_eq!(west[0], "");
    assert_eq!(row("West", "Total", "cost")[..2], ["6", "1"]);
}

#[test]
fn weighted_statistics_agree_with_means() {
    let (tables, _) = run("WEIGHT BY w.
        OLAP CUBES revenue cost BY store BY year /CELLS=MEAN COUNT STDDEV SUM SEMEAN HARMONIC GEOMETRIC.");
    // PSPP MEANS weighted: revenue 13 of 15, cost 14 of 15
    expect(
        titled(&tables, "Case Processing Summary"),
        &[
            &[
                "revenue * store * year",
                "13",
                "86.6667",
                "2",
                "13.3333",
                "15",
                "100",
            ],
            &[
                "cost * store * year",
                "14",
                "93.3333",
                "1",
                "6.6667",
                "15",
                "100",
            ],
        ],
    );
    let report = titled(&tables, "OLAP Cubes");
    let row = |store: &str, year: &str, variable: &str| {
        rows(report)
            .into_iter()
            .find(|r| r[0] == store && r[1] == year && r[2] == variable)
            .unwrap()[3..]
            .to_vec()
    };
    // PSPP: North total revenue mean 14, N 5, sd 3.741657, sum 70, se 1.673320, harmonic
    // 13.291139, geometric 13.629925
    assert_eq!(
        row("North", "Total", "revenue"),
        ["14", "5", "3.7417", "70", "1.6733", "13.2911", "13.6299"]
    );
    // PSPP: all cost mean 4.714286, N 14, sd 2.554892, sum 66, se .682824, harmonic 2.902748,
    // geometric 3.854911
    assert_eq!(
        row("Total", "Total", "cost"),
        ["4.7143", "14", "2.5549", "66", "0.6828", "2.9027", "3.8549"]
    );
}

#[test]
fn group_differences_with_labels_hidden_counts_title_and_footnote() {
    let (tables, _) = run("WEIGHT BY w.
        OLAP CUBES revenue BY store BY year /CELLS=MEAN SUM COUNT
        /CREATE 'Change' DEFAULT GAC GPC (year (2024 2025))
        /TITLE='Weighted' 'by year' /FOOTNOTE='Invented data'
        /HIDESMALLCOUNTS COUNT=3.");
    let report = titled(&tables, "Weighted\nby year");
    assert_eq!(report.caption.as_deref(), Some("Invented data"));
    let all = rows(report);
    let row = |store: &str, year: &str| {
        all.iter()
            .find(|r| r[0] == store && r[1] == year)
            .unwrap_or_else(|| panic!("no row {store} {year}"))[3..]
            .to_vec()
    };
    // North 2024: 10 and 14 (weight 2): mean 38/3, N 3; 2025: 12 and 20, N 2 (hidden)
    assert_eq!(row("North", "2024"), ["12.6667", "38", "3"]);
    assert_eq!(row("North", "2025"), ["16", "32", "<3"]);
    // GAC: 16 - 12.6667 = 3.3333, 32 - 38 = -6, the count difference hidden as one side is
    assert_eq!(row("North", "Change"), ["3.3333", "-6", ""]);
    // GPC: 100 * 3.3333 / 12.6667 = 26.3158, 100 * -6 / 38 = -15.7895
    assert_eq!(row("North", "2025 - 2024 (%)"), ["26.3158", "-15.7895", ""]);
    // all stores: 2024 mean 51/5, N 5; 2025 mean 116/8, N 8: change 4.3, 65, 3;
    // percent 42.1569, 127.451, 60
    assert_eq!(row("Total", "Change"), ["4.3", "65", "3"]);
    assert_eq!(
        row("Total", "2025 - 2024 (%)"),
        ["42.1569", "127.451", "60"]
    );
    // East has one case per year: both differences shown; West has 2025 only: none
    assert!(all.iter().any(|r| r[0] == "East" && r[1] == "Change"));
    assert!(!all.iter().any(|r| r[0] == "West"));
    // the difference rows follow the Total of year
    let north: Vec<&str> = all
        .iter()
        .filter(|r| r[0] == "North")
        .map(|r| r[1].as_str())
        .collect();
    assert_eq!(
        north,
        ["2024", "2025", "Total", "Change", "2025 - 2024 (%)"]
    );
    let gpc = all.iter().position(|r| r[1] == "2025 - 2024 (%)").unwrap();
    assert_eq!(report.formats[gpc][3], "PCT40.1");
}

#[test]
fn variable_differences_in_every_group() {
    let (tables, _) = run("OLAP CUBES revenue cost BY store /CELLS=SUM MEAN
        /CREATE VAC VPC (revenue cost).");
    let all = rows(titled(&tables, "OLAP Cubes"));
    let north: Vec<Vec<String>> = all.into_iter().filter(|r| r[0] == "North").collect();
    // North revenue 10 14 12 20 (sum 56, mean 14), cost 4 6 5 (sum 15, mean 5):
    // cost - revenue = -41 and -9; as percent of revenue -73.2143 and -64.2857
    assert_eq!(
        north,
        [
            ["North", "revenue", "56", "14"],
            ["North", "cost", "15", "5"],
            ["North", "cost - revenue", "-41", "-9"],
            ["North", "cost - revenue (%)", "-73.2143", "-64.2857"],
        ]
    );
}

#[test]
fn percentages_within_a_grouping_variable() {
    let (tables, warnings) =
        run("OLAP CUBES revenue BY store year /CELLS=COUNT NPCT(store) SPCT(store).");
    // in the store table, a store's own share of itself is 100; the Total row too
    let reports: Vec<&Table> = tables.iter().filter(|t| t.title == "OLAP Cubes").collect();
    assert_eq!(reports.len(), 2);
    expect(
        reports[0],
        &[
            &["North", "revenue", "4", "100", "100"],
            &["South", "revenue", "3", "100", "100"],
            &["East", "revenue", "2", "100", "100"],
            &["Total", "revenue", "9", "100", "100"],
        ],
    );
    // the year table has no store: those cells are empty, with a warning
    // 2024 revenue: 10 14 8 5
    assert_eq!(rows(reports[1])[0][2..], ["4", "", ""]);
    assert!(warnings.iter().any(|w| w.contains("within store")));
}

#[test]
fn nested_percentages_within_the_outer_variable() {
    let (tables, _) = run("OLAP CUBES revenue BY store BY year /CELLS=SUM SPCT(store) SPCT.");
    let all = rows(titled(&tables, "OLAP Cubes"));
    // North 2024: 10 + 14 = 24 of North's 56 (42.8571) and of all 112 (21.4286)
    assert_eq!(
        all[0],
        ["North", "2024", "revenue", "24", "42.8571", "21.4286"]
    );
}

#[test]
fn syntax_rules_and_errors() {
    // abbreviations of the command, subcommands and keywords
    let (tables, warnings) = run("OLAP CUB revenue BY store /CEL=MEAN /CELLS=DEFAULT /HIDESMALL.");
    assert_eq!(titled(&tables, "OLAP Cubes").columns.len(), 8);
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("CELLS is given more than once"))
    );
    // HIDESMALLCOUNTS without COUNT hides below 5: East's 2 and South's 3
    let report = titled(&tables, "OLAP Cubes");
    assert_eq!(cell(&report.rows[1][3]), "<5");
    assert!(fails("OLAP CUBES revenue.").contains("BY"));
    assert!(fails("OLAP CUBES revenue BY store BY store.").contains("two BY"));
    assert!(
        fails("DATA LIST FREE /s (A3).\nBEGIN DATA\nab\nEND DATA.\nOLAP CUBES s BY s.")
            .contains("numeric")
    );
    assert!(
        fails("OLAP CUBES revenue BY store /CREATE GAC (year (2024 2025)).")
            .contains("not a grouping variable")
    );
    assert!(
        fails("OLAP CUBES revenue BY store /CREATE GAC (store ('a' 'b')).").contains("numbers")
    );
    assert!(
        fails("OLAP CUBES revenue BY store /CREATE VAC (revenue cost).")
            .contains("summary variable")
    );
    assert!(
        fails("OLAP CUBES revenue cost BY store /CREATE GAC VAC (revenue cost).").contains("mix")
    );
    assert!(
        fails("OLAP CUBES revenue BY store /CREATE 'a' 'b' GAC (store (1 2)).")
            .contains("2 labels for 1")
    );
    assert!(fails("OLAP CUBES revenue BY store /HIDESMALLCOUNTS COUNT=1.").contains("2 or more"));
    assert!(
        fails("OLAP CUBES revenue BY store /CELLS=MEAN NPCT(year).").contains("grouping variable")
    );
    // a category with no cases gives a warning and no difference rows
    let (tables, warnings) =
        run("OLAP CUBES revenue BY store /CELLS=MEAN /CREATE GAC (store (1 9)).");
    assert_eq!(rows(titled(&tables, "OLAP Cubes")).len(), 4);
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("no cases with the value 9"))
    );
}
