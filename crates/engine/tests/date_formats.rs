//! Dates, times and custom currencies in the output of every procedure: a statistic that lies on
//! the variable's scale (mean, median, mode, minimum, maximum, percentile) shows in the
//! variable's format; a spread of dates is a length of time (DTIME); what no date or time
//! shows (a sum or variance of dates, a variance of times) is a plain number. Custom
//! currencies follow SET CCA to CCE. The texts of the dates and times agree with PSPP for
//! the values and minima and maxima, which it shows in the variable's format too.

use statscraft_engine::{Engine, Table, cell_text};
use statscraft_syntax::parse;

/// 2020-01-01, 2021-03-15 and 2022-06-30; 01:00:00, 02:30:30 and 10:20:59; 1, 2 and 4.
const DATA: &str = "DATA LIST LIST /d (ADATE10) t c.
BEGIN DATA
01/01/2020 3600 1
03/15/2021 9030 2
06/30/2022 37259 4
END DATA.
FORMATS t (TIME8) c (CCA8.2).
SET CCA='(,$,,)'.
";

fn run(source: &str) -> Vec<Table> {
    let mut engine = Engine::default();
    let (commands, errors) = parse(&format!("{DATA}{source}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut tables = Vec::new();
    for c in &commands {
        tables.extend(engine.execute(&c.command).unwrap());
    }
    tables
}

/// The text of every cell of a row.
fn row(t: &Table, r: usize) -> Vec<String> {
    (0..t.columns.len()).map(|c| cell_text(t, r, c)).collect()
}

/// The text of every cell of a column.
fn column(t: &Table, c: usize) -> Vec<String> {
    (0..t.rows.len()).map(|r| cell_text(t, r, c)).collect()
}

/// A cell without a format, rounded to a whole number against the rounding error of a sum of
/// squares of numbers near 1e10.
fn variance(t: &Table, r: usize, c: usize) -> f64 {
    assert!(t.formats.is_empty() || t.formats[r][c].is_empty());
    match t.rows[r][c] {
        statscraft_engine::Cell::Number(n) => n.round(),
        _ => panic!("no number"),
    }
}

#[test]
fn list_shows_dates_times_and_currencies() {
    let t = &run("LIST.")[0];
    assert_eq!(row(t, 0), ["01/01/2020", "01:00:00", "$1.00"]);
    assert_eq!(row(t, 2), ["06/30/2022", "10:20:59", "$4.00"]);
}

#[test]
fn frequencies_show_values_and_statistics_in_the_format() {
    let tables = run(
        "FREQUENCIES d t c /STATISTICS=MEAN MEDIAN MODE STDDEV VARIANCE RANGE \
         MINIMUM MAXIMUM SUM SEMEAN SKEWNESS /PERCENTILES=50.",
    );
    let s = &tables[0];
    // N valid, N missing, then the statistics in the order of the manual's keyword list
    let labels = column(s, 0);
    let at = |name: &str| labels.iter().position(|l| l == name).unwrap();
    let d = |name: &str| cell_text(s, at(name), 2);
    let t = |name: &str| cell_text(s, at(name), 3);
    let c = |name: &str| cell_text(s, at(name), 4);
    assert_eq!(cell_text(s, 0, 2), "3");
    assert_eq!(d("Mean"), "03/26/2021");
    assert_eq!(d("Median"), "03/15/2021");
    assert_eq!(d("Mode"), "01/01/2020");
    assert_eq!(d("Minimum"), "01/01/2020");
    assert_eq!(d("Maximum"), "06/30/2022");
    assert_eq!(d("Std. Deviation"), "455 14:23:25");
    assert_eq!(d("Range"), "911 00:00:00");
    assert_eq!(d("Std. Error of Mean"), "263 00:58:23");
    // seconds since 1582 summed or squared mean nothing as a date
    assert_eq!(d("Sum"), "41508288000");
    assert_eq!(t("Mean"), "04:37:09");
    assert_eq!(t("Std. Deviation"), "05:01:10");
    assert_eq!(t("Range"), "09:20:59");
    assert_eq!(t("Sum"), "13:51:29");
    assert_eq!(t("Variance"), "326548270.33");
    assert_eq!(c("Mean"), "$2.33");
    assert_eq!(c("Variance"), "$2.33");
    assert!(!c("Skewness").contains('$'));
    assert_eq!(cell_text(s, s.rows.len() - 1, 2), "03/15/2021");
    // the frequency table of d: its values as dates
    assert_eq!(
        column(&tables[1], 1)[..3],
        ["01/01/2020", "03/15/2021", "06/30/2022"]
    );
    assert_eq!(column(&tables[3], 1)[..2], ["$1.00", "$2.00"]);
}

#[test]
fn descriptives_means_and_examine_follow_the_same_rules() {
    let t = &run("DESCRIPTIVES d t c /STATISTICS=MEAN STDDEV VARIANCE MIN MAX.")[0];
    assert_eq!(
        t.columns[3..],
        ["Mean", "Std Dev", "Variance", "Minimum", "Maximum"]
    );
    let d = row(t, 0);
    assert_eq!(d[2..5], ["3", "03/26/2021", "455 14:23:25"]);
    assert_eq!(d[6..], ["01/01/2020", "06/30/2022"]);
    // the variance of dates, 1549509212160000 square seconds, is a plain number
    assert_eq!(variance(t, 0, 5), 1_549_509_212_160_000.0);
    assert_eq!(row(t, 1)[3..5], ["04:37:09", "05:01:10"]);
    assert_eq!(row(t, 2)[3..5], ["$2.33", "$1.53"]);

    let tables = run("MEANS t c BY d /CELLS=MEAN COUNT STDDEV MIN.");
    let t = &tables[1];
    assert_eq!(t.title, "Report: t by d");
    // the groups of a date control are labelled with their dates
    assert_eq!(row(t, 0), ["01/01/2020", "01:00:00", "1", "", "01:00:00"]);
    assert_eq!(
        row(t, 3),
        ["Total", "04:37:09", "3", "05:01:10", "01:00:00"]
    );
    assert_eq!(
        row(&tables[2], 3),
        ["Total", "$2.33", "3", "$1.53", "$1.00"]
    );

    let tables = run("EXAMINE t /PERCENTILES(50) /STATISTICS=DESCRIPTIVES EXTREME(1).");
    let descriptives = tables.iter().find(|t| t.title == "Descriptives").unwrap();
    let statistic = |name: &str| {
        let r = (0..descriptives.rows.len())
            .find(|&r| cell_text(descriptives, r, 1) == name)
            .unwrap();
        row(descriptives, r)[3..].to_vec()
    };
    assert_eq!(statistic("Mean"), ["04:37:09", "02:53:53"]);
    assert_eq!(statistic("Median"), ["02:30:30", ""]);
    assert_eq!(statistic("Variance"), ["326548270.33", ""]);
    // a lower confidence limit below zero: -07:51:xx does not fit TIME8, so hh:mm
    assert_eq!(statistic("95% Confidence Interval for Mean")[0], "-07:51");
    assert_eq!(statistic("Range"), ["09:20:59", ""]);
    // skewness has no unit
    assert!(!statistic("Skewness")[0].contains(':'));
    let extremes = tables.iter().find(|t| t.title == "Extreme Values").unwrap();
    assert_eq!(
        column(extremes, extremes.columns.len() - 1),
        ["10:20:59", "01:00:00"]
    );
}

#[test]
fn ctables_summaries_take_date_time_and_currency_formats() {
    let t = &run(
        "CTABLES /TABLE d [MEAN, MINIMUM, STDDEV, VARIANCE] + t [MEAN, MEAN TIME5, \
         VARIANCE] + c [S][MEAN, SUM].",
    )[0];
    assert_eq!(
        row(t, 0)[1..4],
        ["03/26/2021", "01/01/2020", "455 14:23:25"]
    );
    assert_eq!(variance(t, 0, 4), 1_549_509_212_160_000.0);
    // stacked variables share the columns of equal summaries
    assert_eq!(row(t, 1)[1], "04:37:09");
    assert_eq!(row(t, 1)[4..6], ["326548270.33", "04:37"]);
    assert_eq!([row(t, 2)[1].as_str(), &row(t, 2)[6]], ["$2.33", "$7.00"]);
}

/// Every header and cell of the tables, as shown.
fn texts(tables: &[Table]) -> Vec<String> {
    let mut out = Vec::new();
    for t in tables {
        out.extend(t.columns.iter().cloned());
        out.extend((0..t.rows.len()).flat_map(|r| row(t, r)));
    }
    out
}

#[test]
fn groups_and_categories_show_in_their_date_or_currency_format() {
    let groups = "DATA LIST LIST /y (F2.0) g (ADATE10) k (F2.0).
BEGIN DATA
1 01/01/2020 1
2 01/01/2020 1
3 03/15/2021 2
5 03/15/2021 2
END DATA.
FORMATS k (CCA8.2).
SET CCA='(,$,,)'.
";
    let has = |source: &str, wanted: &[&str]| {
        let mut engine = Engine::default();
        let (commands, errors) = parse(&format!("{groups}{source}"));
        assert!(errors.is_empty(), "{errors:?}");
        let mut tables = Vec::new();
        for c in &commands {
            tables.extend(engine.execute(&c.command).unwrap());
        }
        let texts = texts(&tables);
        for text in wanted {
            assert!(
                texts.iter().any(|t| t == text),
                "{source}: no {text} in {texts:?}"
            );
        }
        // 2020-01-01 as seconds and the currency group as a bare number never show
        assert!(
            !texts.iter().any(|t| t.contains("13797")),
            "{source}: {texts:?}"
        );
    };
    has(
        "ONEWAY y BY g /STATISTICS DESCRIPTIVES.",
        &["01/01/2020", "03/15/2021"],
    );
    has(
        "ONEWAY y BY k /STATISTICS DESCRIPTIVES.",
        &["$1.00", "$2.00"],
    );
    has("T-TEST GROUPS=k(1 2) /VARIABLES=y.", &["$1.00", "$2.00"]);
    has("T-TEST GROUPS=k(2) /VARIABLES=y.", &[">= $2.00", "< $2.00"]);
    has(
        "T-TEST GROUPS=g(13797216000) /VARIABLES=y.",
        &[">= 01/01/2020"],
    );
    has("CROSSTABS g BY k.", &["01/01/2020", "$1.00", "$2.00"]);
    has("MEANS y BY k.", &["$1.00", "$2.00"]);
    has("EXAMINE y BY k.", &["$1.00", "$2.00"]);
    has("CTABLES /TABLE k [COUNT].", &["$1.00", "$2.00"]);
    has("SPLIT FILE BY k.\nDESCRIPTIVES y.", &["$1.00", "$2.00"]);
}

#[test]
fn a_custom_currency_needs_three_commas_or_three_periods() {
    let mut engine = Engine::default();
    let (commands, _) = parse("SET CCB='-,$'.");
    let error = engine.execute(&commands[0].command).unwrap_err();
    assert!(error.0.contains("three commas or three periods"), "{error}");
    // the value keeps its case: Dfl is not DFL
    let t =
        &run("SET CCB='-/-.Dfl ..-'.\nFORMATS c (CCB16.2).\nCOMPUTE c = c * -1000.\nLIST c.")[0];
    assert_eq!(
        column(t, 0),
        ["-/-Dfl 1.000,00-", "-/-Dfl 2.000,00-", "-/-Dfl 4.000,00-"]
    );
}
