//! FREQUENCIES on small inline datasets. The expected numbers follow the Algorithms manual and
//! are worked out by hand; the unweighted and weighted statistics and percentiles agree with
//! the reference program (`reference_procedures.rs`), which has no grouped percentiles.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

/// x: 10 20 20 30 40 50 60 with weights 1 2 1 1 3 1 1, s: strings, m: 99 is user-missing.
const DATA: &str = "DATA LIST LIST /x w m (F3) s (A3).
BEGIN DATA
10 1 1 'b'
20 2 2 'a'
20 1 99 'a'
30 1 2 'c'
40 3 . 'b'
50 1 3 'a'
60 1 3 'd'
END DATA.
MISSING VALUES m (99).
";

fn run(source: &str) -> (Vec<Table>, Vec<String>) {
    let (commands, errors) = parse(&format!("{DATA}{source}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        tables = engine
            .execute(&command.command)
            .unwrap_or_else(|e| panic!("line {}: {e}", command.first_line));
    }
    (tables, engine.warnings().to_vec())
}

fn number(cell: &Cell) -> Option<f64> {
    match cell {
        Cell::Number(n) => Some(*n),
        Cell::Empty => None,
        other => panic!("not a number: {other:?}"),
    }
}

fn text(cell: &Cell) -> String {
    match cell {
        Cell::Text(t) => t.clone(),
        Cell::Number(n) => n.to_string(),
        Cell::Empty => String::new(),
    }
}

/// The column of one variable in a Statistics table, keyed by the row labels.
fn statistics(table: &Table, variable: &str) -> Vec<(String, Option<f64>)> {
    assert_eq!(table.title, "Statistics");
    let column = table.columns.iter().position(|c| c == variable).unwrap();
    table
        .rows
        .iter()
        .map(|row| {
            let label = format!("{} {}", text(&row[0]), text(&row[1]));
            (label.trim().to_string(), number(&row[column]))
        })
        .collect()
}

fn value(column: &[(String, Option<f64>)], label: &str) -> Option<f64> {
    column
        .iter()
        .find(|(l, _)| l == label)
        .unwrap_or_else(|| panic!("no row {label} in {column:?}"))
        .1
}

fn close(got: Option<f64>, want: f64, label: &str) {
    let got = got.unwrap_or_else(|| panic!("{label}: empty, want {want}"));
    assert!((got - want).abs() < 1e-9, "{label}: {got}, want {want}");
}

#[test]
fn weighted_statistics_follow_the_algorithms() {
    let (tables, _) = run("WEIGHT BY w.\nFREQUENCIES x /STATISTICS=ALL /FORMAT=NOTABLE.");
    assert_eq!(tables.len(), 1);
    let column = statistics(&tables[0], "x");
    // W = 10, mean 33, M2 = 2210
    for (label, want) in [
        ("N Valid", 10.0),
        ("N Missing", 0.0),
        ("Mean", 33.0),
        ("Std. Error of Mean", 4.955_356_249_106_168),
        ("Median", 35.0),
        ("Mode", 20.0),
        ("Std. Deviation", 15.670_212_364_724_211),
        ("Variance", 245.555_555_555_555_56),
        ("Skewness", 0.246_887_418_7),
        ("Std. Error of Skewness", 0.687_042_918_6),
        ("Kurtosis", -0.763_936_622_3),
        ("Std. Error of Kurtosis", 1.334_248_770_0),
        ("Range", 50.0),
        ("Minimum", 10.0),
        ("Maximum", 60.0),
        ("Sum", 330.0),
    ] {
        // the reference program's ten decimals
        let got = value(&column, label).unwrap();
        assert!((got - want).abs() < 1e-9, "{label}: {got}, want {want}");
    }
    // 20 and 40 both weigh 3: the smaller is the mode, and a note says so
    assert!(tables[0].caption.as_ref().unwrap().contains("(x)"));
}

#[test]
fn percentiles_use_w_plus_one() {
    // unweighted: tc = 8 p; weighted: tc = 11 p
    let (tables, _) = run("FREQUENCIES x /PERCENTILES=0 1 10 25 50 75 90 99 100 /FORMAT=NOTABLE.");
    let column = statistics(&tables[0], "x");
    for (p, want) in [
        ("0", 10.0),
        ("1", 10.0),
        ("10", 10.0),
        ("25", 20.0),
        ("50", 30.0),
        ("75", 50.0),
        ("90", 60.0),
        ("99", 60.0),
        ("100", 60.0),
    ] {
        close(value(&column, &format!("Percentiles {p}")), want, p);
    }
    let (tables, _) =
        run("WEIGHT BY w.\nFREQUENCIES x /PERCENTILES=1 10 25 50 75 90 99 /FORMAT=NOTABLE.");
    let column = statistics(&tables[0], "x");
    for (p, want) in [
        ("1", 10.0),
        ("10", 11.0),
        ("25", 20.0),
        ("50", 35.0),
        ("75", 42.5),
        ("90", 59.0),
        ("99", 60.0),
    ] {
        close(value(&column, &format!("Percentiles {p}")), want, p);
    }
    // fractional weights: g is not divided by a weight below 1 (tc = 4 * 0.1 = 0.4, cc_1 = 0.3)
    let (tables, _) = run(
        "COMPUTE w3 = w * 0.3.\nWEIGHT BY w3.\nFREQUENCIES x /PERCENTILES=10 25 /FORMAT=NOTABLE.",
    );
    let column = statistics(&tables[0], "x");
    close(value(&column, "Percentiles 10"), 11.0, "10");
    close(value(&column, "Percentiles 25"), 17.0, "25");
}

#[test]
fn ntiles_add_their_cut_points() {
    let (tables, _) = run("FREQUENCIES x /NTILES=3 /NTILES=4 /FORMAT=NOTABLE.");
    let labels: Vec<String> = statistics(&tables[0], "x")
        .into_iter()
        .map(|(l, _)| l)
        .collect();
    assert_eq!(
        labels,
        [
            "N Valid",
            "N Missing",
            "Percentiles 25",
            "Percentiles 33.333333",
            "Percentiles 50",
            "Percentiles 66.666667",
            "Percentiles 75"
        ]
    );
}

#[test]
fn grouped_percentiles() {
    let percentiles = "/PERCENTILES=0 5 10 25 50 80 90 95 100 /STATISTICS=MEDIAN /FORMAT=NOTABLE.";
    let grouped = |spec: &str| {
        let (tables, warnings) = run(&format!("FREQUENCIES x /GROUPED=x {spec} {percentiles}"));
        (statistics(&tables[0], "x"), warnings)
    };
    // midpoints: cc = 0.5 2 3.5 4.5 5.5 6.5, W = 7
    let (column, warnings) = grouped("");
    close(value(&column, "Median"), 30.0, "median");
    close(
        value(&column, "Percentiles 25"),
        10.0 + 10.0 * 1.25 / 1.5,
        "25",
    );
    close(value(&column, "Percentiles 90"), 58.0, "90");
    for undefined in ["0", "5", "95", "100"] {
        assert_eq!(value(&column, &format!("Percentiles {undefined}")), None);
    }
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    assert!(warnings[0].contains("lower bound") && warnings[1].contains("upper bound"));
    // width 10: groups 5-15 (1), 15-25 (2), 25-35, 35-45, 45-55, 55-65 (1 each)
    let (column, warnings) = grouped("(10)");
    assert!(warnings.is_empty(), "{warnings:?}");
    for (label, want) in [
        ("Median", 30.0),
        ("Percentiles 0", 5.0),
        ("Percentiles 25", 18.75),
        ("Percentiles 90", 58.0),
        ("Percentiles 100", 65.0),
    ] {
        close(value(&column, label), want, label);
    }
    // boundaries 15 35 55: below 15 (1), 15-35 (3), 35-55 (2), above 55 (1)
    let (column, warnings) = grouped("(15, 35, 55)");
    close(value(&column, "Median"), 15.0 + 20.0 * 2.5 / 3.0, "median");
    close(value(&column, "Percentiles 80"), 51.0, "80");
    assert_eq!(value(&column, "Percentiles 10"), None);
    assert_eq!(value(&column, "Percentiles 95"), None);
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    // the mean is not grouped
    let (tables, _) = run("FREQUENCIES x /GROUPED=x (10) /STATISTICS=MEAN /FORMAT=NOTABLE.");
    close(
        value(&statistics(&tables[0], "x"), "Mean"),
        230.0 / 7.0,
        "mean",
    );
}

#[test]
fn grouped_variables_must_be_listed() {
    let (commands, _) = parse(&format!("{DATA}FREQUENCIES x /GROUPED=w."));
    let mut engine = Engine::default();
    let mut last = Ok(Vec::new());
    for command in &commands {
        last = engine.execute(&command.command);
    }
    assert!(last.unwrap_err().to_string().contains("GROUPED names w"));
}

/// (category, value, frequency) of every row of a frequency table.
fn rows(table: &Table) -> Vec<(String, String, f64)> {
    table
        .rows
        .iter()
        .map(|r| (text(&r[0]), text(&r[1]), number(&r[3]).unwrap()))
        .collect()
}

#[test]
fn table_orders() {
    let order = |format: &str| {
        let (tables, warnings) = run(&format!("FREQUENCIES m /FORMAT={format}."));
        let values: Vec<String> = rows(&tables[0]).into_iter().map(|r| r.1).collect();
        (values, warnings)
    };
    // valid 1 (1), 2 (2), 3 (2); missing 99 (1) and system-missing (1)
    assert_eq!(order("AVALUE").0, ["1", "2", "3", "99", "", ""]);
    assert_eq!(order("DVALUE").0, ["3", "2", "1", "99", "", ""]);
    assert_eq!(order("AFREQ").0, ["1", "2", "3", "99", "", ""]);
    assert_eq!(order("DFREQ").0, ["2", "3", "1", "99", "", ""]);
    // ignored with percentiles, and then the median is kept
    let (tables, warnings) = run("FREQUENCIES m /FORMAT=DFREQ /PERCENTILES=50 /STATISTICS=MEDIAN.");
    assert!(warnings[0].contains("ignored"), "{warnings:?}");
    assert_eq!(rows(&tables[1])[0].1, "1");
    close(value(&statistics(&tables[0], "m"), "Median"), 2.0, "median");
    // the median is left out with AFREQ
    let (tables, warnings) = run("FREQUENCIES m /FORMAT=AFREQ /STATISTICS=MEDIAN MEAN.");
    assert!(warnings[0].contains("median"), "{warnings:?}");
    let labels: Vec<String> = statistics(&tables[0], "m")
        .into_iter()
        .map(|r| r.0)
        .collect();
    assert_eq!(labels, ["N Valid", "N Missing", "Mean"]);
    // strings in reverse alphabetical order
    let (tables, _) = run("FREQUENCIES s /FORMAT=DVALUE.");
    let values: Vec<String> = rows(&tables[0]).into_iter().map(|r| r.1).collect();
    assert_eq!(values, ["d", "c", "b", "a", ""]);
}

#[test]
fn percentages_and_include() {
    let (tables, _) = run("FREQUENCIES m.");
    assert_eq!(tables.len(), 1, "no Statistics table without statistics");
    let table = &tables[0];
    // valid percent of 5 valid cases, percent of 7
    let row = &table.rows[1];
    close(number(&row[4]), 200.0 / 7.0, "percent");
    close(number(&row[5]), 40.0, "valid percent");
    close(number(&row[6]), 60.0, "cumulative percent");
    let (tables, _) = run("FREQUENCIES m /MISSING=INCLUDE /STATISTICS=MEAN.");
    let column = statistics(&tables[0], "m");
    close(value(&column, "N Valid"), 6.0, "valid");
    close(value(&column, "N Missing"), 1.0, "missing");
    close(value(&column, "Mean"), 110.0 / 6.0, "mean");
    let categories: Vec<String> = rows(&tables[1]).into_iter().map(|r| r.0).collect();
    assert_eq!(
        categories,
        ["valid", "valid", "valid", "valid", "missing", "total"]
    );
}

#[test]
fn limit_notable_and_order() {
    // LIMIT counts every row of values, missing ones too: m has 5, x 6
    let (tables, _) = run("FREQUENCIES m x /FORMAT=LIMIT(5).");
    let titles: Vec<&str> = tables.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(titles, ["Statistics", "m"]);
    assert_eq!(tables[0].columns, ["Statistic", "", "m", "x"]);
    let (tables, _) = run("FREQUENCIES m x s /FORMAT=NOTABLE.");
    assert_eq!(tables.len(), 1);
    // a string variable gets its counts only
    let column = statistics(&tables[0], "s");
    close(value(&column, "N Valid"), 7.0, "valid");
    let (tables, _) = run("FREQUENCIES m x /ORDER=VARIABLE /STATISTICS=MEAN.");
    let titles: Vec<&str> = tables.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(titles, ["Statistics", "m", "Statistics", "x"]);
    assert_eq!(tables[2].columns, ["Statistic", "", "x"]);
}

#[test]
fn a_histogram_keeps_the_values_ascending() {
    let (tables, warnings) = run("FREQUENCIES x /HISTOGRAM=NORMAL /PIECHART /FORMAT=DVALUE.");
    assert_eq!(tables.len(), 1);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("ignored"), "{warnings:?}");
    assert_eq!(rows(&tables[0])[0].1, "10");
}

#[test]
fn statistics_need_enough_cases() {
    let (tables, _) = run("SELECT IF x <= 20.\nFREQUENCIES x /STATISTICS=ALL /FORMAT=NOTABLE.");
    let column = statistics(&tables[0], "x");
    // W = 3: skewness yes, kurtosis no
    assert!(value(&column, "Skewness").is_some());
    assert_eq!(value(&column, "Kurtosis"), None);
    assert_eq!(value(&column, "Std. Error of Kurtosis"), None);
    let (tables, _) = run("SELECT IF x = 10.\nFREQUENCIES x /STATISTICS=ALL /FORMAT=NOTABLE.");
    let column = statistics(&tables[0], "x");
    assert_eq!(value(&column, "Variance"), None);
    assert_eq!(value(&column, "Std. Error of Mean"), None);
    close(value(&column, "Mean"), 10.0, "mean");
}
