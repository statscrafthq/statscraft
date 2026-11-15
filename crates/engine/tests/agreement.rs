//! WEIGHTED KAPPA, FLEISS MULTIRATER KAPPA and RATIO STATISTICS on small invented data.
//! Expected kappas and z values were checked against a general statistics package run as a
//! black box; the ratio statistics were worked out by hand from the formulas.

use statscraft_engine::session::Session;
use statscraft_engine::{Cell, Table};

/// Two nurses score the pain of twelve patients from 1 to 4; a doctor too, as text.
const PAIN: &str = "DATA LIST FREE /nurse_a nurse_b (2F1) doctor (A4).
BEGIN DATA
1 1 low
2 3 mid
3 3 MID
4 4 top
2 2 mid
3 2 high
1 1 low
4 3 high
3 3 high
2 2 Mid
2 1 low
4 4 top
END DATA.
";

/// Four judges sort ten essays into three grades.
const ESSAYS: &str = "DATA LIST FREE /j1 j2 j3 j4.
BEGIN DATA
1 1 1 2
2 2 2 2
3 3 2 3
1 2 1 1
3 3 3 3
2 1 2 3
1 1 1 1
2 2 3 2
3 2 3 3
1 1 2 1
END DATA.
";

/// Rent and income of households in two districts; the last two cases are not used.
const RENT: &str = "DATA LIST FREE /district rent income.
BEGIN DATA
1 300 1000
1 450 1200
1 500 2000
1 380 950
2 600 1500
2 700 2800
2 800 1600
2 900 3000
2 400 .
2 0 1000
END DATA.
VALUE LABELS district 1 'North' 2 'South'.
";

fn run(source: &str) -> (Vec<Table>, Vec<String>) {
    let result = Session::default()
        .run(source, None)
        .unwrap_or_else(|e| panic!("{source}\n{e:?}"));
    let tables = result.commands.iter().flat_map(|c| c.tables.clone());
    let warnings = result.commands.iter().flat_map(|c| c.warnings.clone());
    (
        tables.filter(|t| t.title != "Echo").collect(),
        warnings.collect(),
    )
}

fn fails(source: &str) -> String {
    let errors = Session::default().run(source, None).unwrap_err();
    errors
        .iter()
        .map(|e| e.text())
        .collect::<Vec<_>>()
        .join(" | ")
}

fn titled<'a>(tables: &'a [Table], title: &str) -> &'a Table {
    tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"))
}

fn value(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        other => panic!("{other:?} is no number"),
    }
}

fn close(cell: &Cell, expected: f64) {
    let got = value(cell);
    assert!((got - expected).abs() < 1e-8, "{got} != {expected}");
}

fn texts(table: &Table, column: usize) -> Vec<String> {
    table
        .rows
        .iter()
        .map(|r| match &r[column] {
            Cell::Text(t) => t.clone(),
            other => format!("{other:?}"),
        })
        .collect()
}

#[test]
fn weighted_kappa_linear_and_quadratic() {
    let (tables, _) = run(&format!("{PAIN}WEIGHTED KAPPA nurse_a nurse_b."));
    let t = titled(&tables, "Weighted Kappa");
    assert_eq!(t.columns[2], "Weighted Kappa");
    assert_eq!(
        t.column_groups[6],
        vec!["95% Asymptotic Confidence Interval"]
    );
    let row = &t.rows[0];
    assert_eq!(row[0], Cell::Text("nurse_a - nurse_b".into()));
    close(&row[1], 12.0);
    close(&row[2], 0.7142857143);
    close(&row[3], 0.1301437175);
    close(&row[4], 3.6115755926);
    close(&row[5], 0.0003043423);
    close(&row[6], 0.7142857143 - 1.959963984540054 * 0.1301437175);
    assert!(t.caption.as_ref().unwrap().starts_with("Linear"));

    let (tables, _) = run(&format!(
        "{PAIN}WEIGHTED KAPPA nurse_a nurse_b /CRITERIA WEIGHTING(QUADRATIC) ASYMPTOTIC_CILEVEL=90."
    ));
    let t = titled(&tables, "Weighted Kappa");
    close(&t.rows[0][2], 0.8471337580);
    close(&t.rows[0][3], 0.0794789782);
    close(&t.rows[0][4], 2.9724226762);
    close(
        &t.rows[0][7],
        0.8471337580 + 1.6448536269514722 * 0.0794789782,
    );
    assert_eq!(
        t.column_groups[6],
        vec!["90% Asymptotic Confidence Interval"]
    );
}

#[test]
fn weighted_kappa_pairs_and_tabulation() {
    // one list: every pair once; BY: row by column, never a variable with itself
    let (tables, warnings) = run(&format!(
        "{PAIN}WEIGHTED KAPPA nurse_a nurse_b doctor /PRINT TABULATION(nurse_a BY nurse_b) ORDER(DESCENDING)."
    ));
    let t = titled(&tables, "Weighted Kappa");
    assert_eq!(texts(t, 0), ["nurse_a - nurse_b"]);
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    assert!(warnings[0].contains("not both numeric or both string"));
    let cross = titled(&tables, "nurse_a * nurse_b Crosstabulation");
    assert_eq!(cross.columns, ["nurse_a", "4", "3", "2", "1", "Total"]);
    assert_eq!(texts(cross, 0), ["4", "3", "2", "1", "Total"]);
    // nurse_a 4 with nurse_b 4 twice, with 3 once
    assert_eq!(cross.rows[0][1], Cell::Number(2.0));
    assert_eq!(cross.rows[0][2], Cell::Number(1.0));
    assert_eq!(cross.rows[4][5], Cell::Number(12.0));

    let (tables, _) = run(&format!(
        "{PAIN}WEIGHTED KAPPA nurse_a nurse_b BY nurse_b nurse_a /PRINT TABULATION(ALL)."
    ));
    let t = titled(&tables, "Weighted Kappa");
    assert_eq!(texts(t, 0), ["nurse_a - nurse_b", "nurse_b - nurse_a"]);
    // the kappa does not depend on which rater is the row
    close(&t.rows[1][2], 0.7142857143);
    assert_eq!(tables.len(), 3);
}

#[test]
fn weighted_kappa_strings_and_case() {
    let (tables, _) = run(&format!(
        "{PAIN}STRING doc2 (A4).
COMPUTE doc2 = doctor.
IF ($CASENUM = 1) doc2 = 'mid'.
WEIGHTED KAPPA doctor doc2 /CRITERIA IGNORECASE=TRUE /PRINT TABULATION(ALL) CASE(LOWER)."
    ));
    let cross = titled(&tables, "doctor * doc2 Crosstabulation");
    // HIGH, LOW, MID, TOP after folding the case, shown in lower case
    assert_eq!(texts(cross, 0), ["high", "low", "mid", "top", "Total"]);
    let t = titled(&tables, "Weighted Kappa");
    assert!(t.caption.as_ref().unwrap().contains("regardless of case"));
    // case-sensitive: MID and Mid are categories of their own
    let (tables, _) = run(&format!(
        "{PAIN}STRING doc2 (A4).
COMPUTE doc2 = doctor.
WEIGHTED KAPPA doctor doc2 /PRINT TABULATION(ALL)."
    ));
    let cross = titled(&tables, "doctor * doc2 Crosstabulation");
    assert_eq!(
        texts(cross, 0),
        ["MID", "Mid", "high", "low", "mid", "top", "Total"]
    );
    close(&titled(&tables, "Weighted Kappa").rows[0][2], 1.0);
}

#[test]
fn weighted_kappa_weights_count_as_repeated_cases() {
    let weighted = format!(
        "{PAIN}COMPUTE w = 1.
IF ($CASENUM = 2) w = 3.
WEIGHT BY w.
WEIGHTED KAPPA nurse_a nurse_b."
    );
    let (tables, _) = run(&weighted);
    let repeated = "DATA LIST FREE /nurse_a nurse_b.
BEGIN DATA
1 1 2 3 2 3 2 3 3 3 4 4 2 2 3 2 1 1 4 3 3 3 2 2 2 1 4 4
END DATA.
WEIGHTED KAPPA nurse_a nurse_b.";
    let (expected, _) = run(repeated);
    let (a, b) = (&tables[0].rows[0], &expected[0].rows[0]);
    for i in 1..8 {
        close(&a[i], value(&b[i]));
    }
}

#[test]
fn weighted_kappa_errors() {
    assert!(fails(&format!("{PAIN}WEIGHTED KAPPA nurse_a nurse_a.")).contains("two different"));
    assert!(
        fails(&format!("{PAIN}WEIGHTED KAPPA nurse_a BY nurse_a.")).contains("no two different")
    );
    assert!(
        fails(&format!(
            "{PAIN}WEIGHTED KAPPA nurse_a BY nurse_b /PRINT TABULATION(nurse_b BY nurse_b)."
        ))
        .contains("not on the row list")
    );
}

#[test]
fn fleiss_kappa_overall_and_categories() {
    let (tables, _) = run(&format!(
        "{ESSAYS}VALUE LABELS j1 1 'Fail' 2 'Pass' 3 'Merit'.
FLEISS MULTIRATER KAPPA j1 j2 j3 j4 /PRINT CATEGORY."
    ));
    let overall = titled(&tables, "Overall Agreement");
    let row = &overall.rows[0];
    close(&row[1], 0.4235588972);
    close(&row[3], 4.6334421772);
    close(&row[4], 0.0000035964);
    close(&row[2], 0.4235588972 / 4.6334421772);
    assert_eq!(
        overall.caption.as_deref(),
        Some("10 subjects and 4 raters.")
    );
    let categories = titled(&tables, "Agreement on Individual Categories");
    assert_eq!(texts(categories, 0), ["Fail", "Pass", "Merit"]);
    // grade 1: 14 of 40 ratings, sum of squared counts 44
    let p = 14.0 / 40.0;
    let conditional = (44.0 - 40.0 * p) / (40.0 * 3.0 * p);
    close(&categories.rows[0][1], conditional);
    close(&categories.rows[0][2], (conditional - p) / (1.0 - p));
    close(&categories.rows[0][3], (2.0f64 / 120.0).sqrt());
}

#[test]
fn fleiss_kappa_missing_weights_and_fractions() {
    // a case missing on one rater is dropped; fractions are cut; a weight of 0.5 drops a case
    let (a, _) = run(&format!(
        "{ESSAYS}COMPUTE j1 = j1 + 0.7.
COMPUTE w = 1.
IF ($CASENUM = 3) w = 0.5.
WEIGHT BY w.
FLEISS MULTIRATER KAPPA j1 TO j4."
    ));
    let (b, _) = run(&format!(
        "{ESSAYS}SELECT IF ($CASENUM NE 3).\nFLEISS MULTIRATER KAPPA j1 TO j4."
    ));
    close(&a[0].rows[0][1], value(&b[0].rows[0][1]));
    assert_eq!(a[0].caption, b[0].caption);
    assert_eq!(a[0].caption.as_deref(), Some("9 subjects and 4 raters."));
    let (_, warnings) = run(&format!(
        "{ESSAYS}SELECT IF ($CASENUM = 2).\nFLEISS MULTIRATER KAPPA j1 j2 j4."
    ));
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("fewer than two complete subjects")),
        "{warnings:?}"
    );
    assert!(fails(&format!("{ESSAYS}FLEISS MULTIRATER KAPPA j1 j1 j2.")).contains("named twice"));
    assert!(fails(&format!("{ESSAYS}FLEISS MULTIRATER KAPPA j1.")).contains("at least two"));
}

#[test]
fn ratio_statistics_overall_and_by_group() {
    let (tables, _) = run(&format!(
        "{RENT}RATIO STATISTICS rent WITH income BY district
  /PRINT=MEAN MEDIAN WGTMEAN CIN MIN MAX RANGE STDDEV AAD PRD COD MNCOV MDCOV BCOC(0.3,0.4) WCOC(15)."
    ));
    let summary = titled(&tables, "Case Processing Summary");
    assert_eq!(
        texts(summary, 0),
        ["North", "South", "Overall", "Excluded", "Total"]
    );
    close(&summary.rows[1][1], 4.0);
    close(&summary.rows[3][1], 2.0);
    close(&summary.rows[3][2], 20.0);
    let t = titled(&tables, "Ratio Statistics for rent / income");
    assert_eq!(texts(t, 0), ["North", "South", "Overall"]);
    let overall = &t.rows[2];
    let expected = [
        0.346875,
        0.27409735,
        0.41965265, // mean and interval
        0.3375,
        0.25,
        0.5,
        99.21875, // median, interval, coverage
        0.3295373665,
        0.253918682,
        0.4051560511, // weighted mean and interval
        0.25,
        0.5,
        0.25,
        0.0870524267,
        0.071875,
        1.052612041,
        21.2962962963,
        25.096195094,
        25.9636913151,
        62.5,
        37.5,
    ];
    assert_eq!(overall.len(), expected.len() + 1);
    for (cell, e) in overall[1..].iter().zip(expected) {
        assert!((value(cell) - e).abs() < 1e-7, "{cell:?} != {e}");
    }
    assert_eq!(t.column_groups[2], vec!["95% Confidence Interval for Mean"]);
    // South: four ratios are too few for a 95% median interval
    assert_eq!(t.rows[1][5], Cell::Empty);
    let south = [0.3625, 0.1860846071, 0.5389153929, 0.35];
    for (cell, e) in t.rows[1][1..5].iter().zip(south) {
        assert!((value(cell) - e).abs() < 1e-7, "{cell:?} != {e}");
    }
    close(&t.rows[0][17], 16.6666666667);
}

#[test]
fn ratio_statistics_order_missing_and_outfile() {
    let (tables, _) = run(&format!(
        "{RENT}MISSING VALUES income (1500).
RATIO STATISTICS rent WITH income BY district (DESCENDING) /MISSING=EXCLUDE /PRINT=MEDIAN."
    ));
    let summary = titled(&tables, "Case Processing Summary");
    assert_eq!(
        texts(summary, 0),
        ["South", "North", "Overall", "Excluded", "Total"]
    );
    close(&summary.rows[0][1], 3.0);
    let (tables, _) = run(&format!(
        "{RENT}MISSING VALUES income (1500).
RATIO STATISTICS rent WITH income /MISSING=INCLUDE /PRINT=MEDIAN."
    ));
    close(&titled(&tables, "Case Processing Summary").rows[0][1], 8.0);
    // no PRINT: only the summary
    let (tables, _) = run(&format!("{RENT}RATIO STATISTICS rent WITH income."));
    assert_eq!(tables.len(), 1);

    let (tables, _) = run(&format!(
        "{RENT}DATASET DECLARE ratios.
RATIO STATISTICS rent WITH income BY district /OUTFILE('ratios')=MEDIAN COD CIN(90).
DATASET ACTIVATE ratios.
LIST."
    ));
    let list = tables.last().unwrap();
    assert_eq!(
        list.columns,
        [
            "district",
            "Median",
            "Median_Lower",
            "Median_Upper",
            "Median_Coverage",
            "COD"
        ]
    );
    assert_eq!(list.rows.len(), 3);
    // the overall record first, its group value system-missing
    assert_eq!(list.rows[0][0], Cell::Text(".".into()));
}

#[test]
fn ratio_statistics_errors() {
    assert!(
        fails(&format!(
            "{RENT}STRING s (A3).\nRATIO STATISTICS s WITH income."
        ))
        .contains("numeric")
    );
    assert!(
        fails(&format!(
            "{RENT}SPLIT FILE BY district.\nRATIO STATISTICS rent WITH income /OUTFILE('x.sav')=MEAN."
        ))
        .contains("SPLIT FILE")
    );
}
