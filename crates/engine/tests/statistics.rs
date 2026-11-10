//! MEANS, T-TEST, ONEWAY and CORRELATIONS on the survey fixtures. Expected numbers come from two
//! sources: the reference program, printed with ten decimals where it follows the manuals
//! (`reference_procedures.rs` repeats those comparisons on many more cases), and an independent
//! computation of the manual's formulas in a general statistics library for what that program
//! does not do (medians, the analysis of variance of MEANS, the Welch and Brown-Forsythe tests,
//! Tamhane's T2, effect sizes).

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

fn fixture(name: &str) -> String {
    format!("{}/../../tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn run(engine: &mut Engine, source: &str) -> Vec<Table> {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut tables = Vec::new();
    for command in &commands {
        match engine.execute(&command.command) {
            Ok(more) => tables.extend(more),
            Err(error) => panic!("line {}: {error}", command.first_line),
        }
    }
    tables
}

fn loaded(name: &str) -> Engine {
    let mut engine = Engine::default();
    run(&mut engine, &format!("GET FILE='{}'.", fixture(name)));
    engine
}

fn survey() -> Engine {
    loaded("survey_basic.sav")
}

/// The survey with `weight` as the weight variable.
fn weighted() -> Engine {
    loaded("survey_basic_phpwriter.sav")
}

fn number(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        other => panic!("not a number: {other:?}"),
    }
}

fn text(cell: &Cell) -> &str {
    match cell {
        Cell::Text(t) => t,
        Cell::Empty => "",
        other => panic!("not text: {other:?}"),
    }
}

/// `actual` against `expected`, which was printed with `decimals` decimals.
fn near(actual: f64, expected: f64, decimals: i32) {
    let allowed = 0.5 * 10f64.powi(-decimals) + 1e-12 * expected.abs();
    assert!(
        (actual - expected).abs() <= allowed,
        "{actual} is not {expected}"
    );
}

fn titled<'t>(tables: &'t [Table], title: &str) -> &'t Table {
    tables
        .iter()
        .find(|t| t.title.starts_with(title))
        .unwrap_or_else(|| panic!("no table {title}"))
}

#[test]
fn means_report_groups_nested_with_totals() {
    let tables = run(
        &mut weighted(),
        "MEANS TABLES=age BY gender BY q5_1 /CELLS=MEAN COUNT STDDEV.",
    );
    assert_eq!(tables.len(), 2);
    let summary = &tables[0];
    assert_eq!(text(&summary.rows[0][0]), "age * gender * q5_1");
    near(number(&summary.rows[0][1]), 259.9341, 4);
    near(number(&summary.rows[0][2]), 100.0, 10);
    let report = &tables[1];
    assert_eq!(report.title, "Report: age by gender by q5_1");
    assert_eq!(report.row_headers, 2);
    assert_eq!(report.columns[2..], ["Mean", "N", "Std. Deviation"]);
    // three genders and the Total, each with two categories and a Total
    assert_eq!(report.rows.len(), 12);
    assert_eq!(
        [text(&report.rows[0][0]), text(&report.rows[0][1])],
        ["Male", "Not selected"]
    );
    assert_eq!(
        [text(&report.rows[11][0]), text(&report.rows[11][1])],
        ["Total", "Total"]
    );
    near(number(&report.rows[0][2]), 52.419_027_613_3, 10);
    near(number(&report.rows[0][4]), 19.395_638_328_5, 10);
    near(number(&report.rows[9][2]), 51.852_627_015_2, 10);
    near(number(&report.rows[11][4]), 19.102_454_972_8, 10);
}

#[test]
fn means_cells_and_analysis_of_variance() {
    let tables = run(
        &mut survey(),
        "MEANS age BY gender /CELLS=MEDIAN NPCT SPCT FIRST LAST /STATISTICS=LINEARITY.",
    );
    let report = titled(&tables, "Report");
    let medians: Vec<f64> = report.rows.iter().map(|r| number(&r[1])).collect();
    assert_eq!(medians, [51.5, 50.0, 46.0, 50.5]);
    // shares of the cases and of the sum of ages add up to 100
    let n: f64 = report.rows[..3].iter().map(|r| number(&r[2])).sum();
    let sum: f64 = report.rows[..3].iter().map(|r| number(&r[3])).sum();
    near(n, 100.0, 12);
    near(sum, 100.0, 12);
    near(number(&report.rows[3][2]), 100.0, 12);
    let anova = titled(&tables, "ANOVA Table");
    let labels: Vec<&str> = anova.rows.iter().map(|r| text(&r[1])).collect();
    assert_eq!(
        labels,
        [
            "(Combined)",
            "Linearity",
            "Deviation from Linearity",
            "",
            ""
        ]
    );
    near(number(&anova.rows[0][2]), 66.544_342_481_418_11, 9);
    near(number(&anova.rows[0][5]), 0.089_178_896_440_770_86, 12);
    near(number(&anova.rows[0][6]), 0.914_718_829_710_286_8, 12);
    near(number(&anova.rows[1][2]), 63.004_032_258_064_4, 9);
    near(number(&anova.rows[3][2]), 73_499.650_657_518_58, 8);
    assert_eq!(number(&anova.rows[3][3]), 197.0);
    assert_eq!(anova.rows[4][4], Cell::Empty);
    let association = titled(&tables, "Measures of Association");
    near(
        number(&association.rows[0][1]),
        -0.029_264_763_960_869_015,
        12,
    );
    near(
        number(&association.rows[0][3]),
        0.030_075_746_948_717_418,
        12,
    );
}

#[test]
fn means_missing_values_per_dependent_or_per_table() {
    // monthly_spend has 19 system-missing values; on its own it leaves out only those
    let tables = run(&mut survey(), "MEANS age monthly_spend BY gender.");
    let summary = &tables[0];
    assert_eq!(number(&summary.rows[0][1]), 200.0);
    assert_eq!(number(&summary.rows[1][1]), 181.0);
    // any /MISSING is tablewise: age loses the cases missing on monthly_spend too
    let tables = run(
        &mut survey(),
        "MEANS age monthly_spend BY gender /MISSING=TABLE.",
    );
    assert_eq!(number(&tables[0].rows[0][1]), 181.0);
    assert_eq!(number(&tables[0].rows[0][3]), 19.0);
    // user-missing 98 and 99 of the control form groups with DEPENDENT, not as a dependent
    let tables = run(
        &mut survey(),
        "MEANS age BY q1_satisfaction /MISSING=DEPENDENT.",
    );
    let groups: Vec<&str> = tables[1].rows.iter().map(|r| text(&r[0])).collect();
    assert!(groups.contains(&"Refused"), "{groups:?}");
    let tables = run(
        &mut survey(),
        "MEANS q1_satisfaction BY gender /MISSING=DEPENDENT.",
    );
    let total = tables[1].rows.last().unwrap();
    assert!(number(&total[1]) < 6.0, "{total:?}");
}

#[test]
fn means_string_dependent_and_warnings() {
    let mut engine = survey();
    let tables = run(
        &mut engine,
        "MEANS plan BY gender /CELLS=COUNT MEAN FIRST LAST /STATISTICS=ANOVA.",
    );
    let report = &tables[1];
    assert_eq!(number(&report.rows[3][1]), 200.0);
    assert_eq!(report.rows[3][2], Cell::Empty);
    assert!(matches!(report.rows[3][3], Cell::Text(_)));
    assert_eq!(
        engine.warnings(),
        ["no analysis of variance for plan by gender: plan is a string variable"]
    );
    run(
        &mut engine,
        "SELECT IF gender = 1.\nMEANS age BY gender /STATISTICS=LINEARITY.",
    );
    assert_eq!(
        engine.warnings(),
        ["no analysis of variance for age by gender: it needs at least two groups"]
    );
}

#[test]
fn t_test_one_sample_with_effect_sizes() {
    let tables = run(&mut survey(), "T-TEST /TESTVAL=50 /VARIABLES=age.");
    let titles: Vec<&str> = tables.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "One-Sample Statistics",
            "One-Sample Test (Test Value = 50)",
            "One-Sample Effect Sizes"
        ]
    );
    let effects = &tables[2];
    assert_eq!(text(&effects.rows[0][1]), "Cohen's d");
    near(number(&effects.rows[0][3]), 0.043_948_504_385_819_76, 12);
    near(number(&effects.rows[0][4]), -0.094_764_160_206_702_18, 10);
    near(number(&effects.rows[0][5]), 0.182_550_916_936_861_32, 10);
    let j = 0.996_225_623_024_769;
    near(
        number(&effects.rows[1][3]),
        0.043_948_504_385_819_76 * j,
        12,
    );
    near(
        number(&effects.rows[1][2]),
        number(&effects.rows[0][2]) / j,
        10,
    );
    let none = run(
        &mut survey(),
        "T-TEST /TESTVAL=50 /VARIABLES=age /ES DISPLAY(FALSE).",
    );
    assert_eq!(none.len(), 2);
}

#[test]
fn t_test_independent_samples_weighted() {
    let tables = run(
        &mut weighted(),
        "T-TEST GROUPS=gender(1,2) /VARIABLES=age /CRITERIA=CI(.90).",
    );
    let groups = &tables[0];
    near(number(&groups.rows[0][2]), 132.3074, 4);
    near(number(&groups.rows[1][3]), 51.007_525_970_3, 10);
    let test = &tables[1];
    // Levene, t, df, significance, difference, its standard error, the 90% interval
    let pooled = [
        1.212_394_973_1,
        0.216_841_675_2,
        249.694,
        0.829,
        0.527_663_293_5,
        2.433_403_510_4,
    ];
    for (column, expected) in [2, 4, 5, 6, 7, 8].into_iter().zip(pooled) {
        let decimals = if column == 6 { 3 } else { 10 };
        near(number(&test.rows[0][column]), expected, decimals);
    }
    near(number(&test.rows[1][5]), 247.685_020_638_3, 10);
    assert_eq!(test.rows[1][2], Cell::Empty);
    assert_eq!(
        test.column_groups[9],
        ["t-test for Equality of Means", "90% Confidence Interval"]
    );
}

#[test]
fn t_test_groups_by_a_cut_point_and_paired_samples() {
    let tables = run(
        &mut survey(),
        "T-TEST GROUPS=age(50) /VARIABLES=monthly_spend.",
    );
    let groups = &tables[0];
    assert_eq!(text(&groups.rows[0][1]), ">= 50");
    assert_eq!(text(&groups.rows[1][1]), "< 50");
    let total = number(&groups.rows[0][2]) + number(&groups.rows[1][2]);
    assert_eq!(total, 181.0);

    let tables = run(
        &mut weighted(),
        "T-TEST PAIRS=q1_satisfaction q5_1 WITH age q5_2 (PAIRED).",
    );
    let correlations = &tables[1];
    assert_eq!(text(&correlations.rows[1][1]), "q5_1 & q5_2");
    near(number(&correlations.rows[0][2]), 230.6772, 4);
    let test = &tables[2];
    near(number(&test.rows[0][2]), -47.192_124_319_2, 10);
    near(number(&test.rows[0][3]), 19.517_268_718_8, 10);
    near(number(&test.rows[1][7]), -2.719_895_423_2, 10);
    near(number(&test.rows[1][8]), 258.9341, 4);
    assert_eq!(tables[3].title, "Paired Samples Effect Sizes");
}

#[test]
fn t_test_warns_about_unmatched_paired_lists() {
    let mut engine = survey();
    let tables = run(&mut engine, "T-TEST PAIRS=q5_1 q5_2 WITH q5_3 (PAIRED).");
    assert_eq!(tables[1].rows.len(), 1);
    assert_eq!(
        engine.warnings(),
        ["(PAIRED) has 2 variables before WITH and 1 after; the unmatched ones are left out"]
    );
}

#[test]
fn oneway_tables_weighted() {
    let tables = run(
        &mut weighted(),
        "ONEWAY age BY region /STATISTICS=DESCRIPTIVES HOMOGENEITY /CONTRAST=1 -1 0 0 0 /POSTHOC=TUKEY.",
    );
    let titles: Vec<&str> = tables.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "Descriptives",
            "Test of Homogeneity of Variances",
            "ANOVA",
            "Contrast Coefficients",
            "Contrast Tests",
            "Multiple Comparisons: age"
        ]
    );
    let descriptives = &tables[0];
    near(number(&descriptives.rows[0][6]), 38.520_011_872_7, 10);
    near(number(&tables[1].rows[0][1]), 0.728_510_347_5, 10);
    let anova = &tables[2];
    near(number(&anova.rows[0][2]), 4_569.257_708_853_7, 10);
    near(number(&anova.rows[0][5]), 3.238_716_003_1, 10);
    let contrasts = &tables[4];
    near(number(&contrasts.rows[0][4]), 3.682_795_537, 10);
    near(number(&contrasts.rows[1][6]), 102.258_710_462_5, 10);
    let tukey = &tables[5];
    near(number(&tukey.rows[0][5]), 0.073, 3);
    near(number(&tukey.rows[2][5]), 0.008, 3);
}

#[test]
fn oneway_robust_tests_and_separate_variance_comparisons() {
    let tables = run(
        &mut survey(),
        "ONEWAY age BY region /STATISTICS=WELCH BROWNFORSYTHE /POSTHOC=T2 GH.",
    );
    let robust = titled(&tables, "Robust Tests");
    near(number(&robust.rows[0][2]), 2.020_783_464_855_474, 12);
    near(number(&robust.rows[0][4]), 92.839_132_951_574_43, 10);
    near(number(&robust.rows[0][5]), 0.097_895_239_057_414_11, 12);
    near(number(&robust.rows[1][2]), 2.074_410_361_603_061_6, 12);
    near(number(&robust.rows[1][4]), 185.929_331_033_706_86, 10);
    let comparisons = titled(&tables, "Multiple Comparisons");
    assert_eq!(text(&comparisons.rows[0][0]), "Tamhane");
    near(number(&comparisons.rows[0][4]), 4.153_416_787_051_074, 12);
    near(number(&comparisons.rows[0][5]), 0.441_067_652_648_411_4, 10);
    let games_howell = &comparisons.rows[20];
    assert_eq!(text(&games_howell[0]), "Games-Howell");
    near(number(&games_howell[5]), 0.307_121_930_382_999, 9);
}

#[test]
fn oneway_contrast_warnings() {
    let mut engine = survey();
    let tables = run(
        &mut engine,
        "ONEWAY age BY gender /CONTRAST=1 1 1 /CONTRAST=1 -1.",
    );
    assert_eq!(tables.len(), 3);
    assert_eq!(
        engine.warnings(),
        [
            "the coefficients of contrast 1 do not sum to 0",
            "contrast 2 has 2 coefficients for 3 groups and is not tested"
        ]
    );
}

#[test]
fn correlations_pairwise_weighted() {
    let tables = run(
        &mut weighted(),
        "CORRELATIONS /VARIABLES=age monthly_spend q1_satisfaction /STATISTICS=ALL.",
    );
    let descriptives = &tables[0];
    near(number(&descriptives.rows[1][2]), 114.040_370_014, 10);
    let correlations = &tables[1];
    let labels: Vec<&str> = correlations.rows[..5].iter().map(|r| text(&r[1])).collect();
    assert_eq!(
        labels,
        [
            "Pearson Correlation",
            "Sig. (2-tailed)",
            "Sum of Squares and Cross-products",
            "Covariance",
            "N"
        ]
    );
    assert_eq!(number(&correlations.rows[0][2]), 1.0);
    assert_eq!(correlations.rows[1][2], Cell::Empty);
    near(number(&correlations.rows[1][3]), 0.561, 3);
    near(number(&correlations.rows[2][3]), 19_439.397_277_621_7, 10);
    near(number(&correlations.rows[3][3]), 83.494_675_235_3, 10);
    near(number(&correlations.rows[9][4]), 206.6982, 4);
}

#[test]
fn correlations_listwise_one_tailed_lower_and_flags() {
    let tables = run(
        &mut survey(),
        "CORRELATIONS /VARIABLES=age monthly_spend q1_satisfaction /MISSING=LISTWISE /PRINT=ONETAIL NOSIG LNODIAG.",
    );
    let table = &tables[0];
    // no N rows: listwise N is the caption
    assert_eq!(table.rows.len(), 6);
    assert_eq!(text(&table.rows[1][1]), "Sig. (1-tailed)");
    let caption = table.caption.as_deref().unwrap();
    assert!(caption.ends_with("Listwise N = 161."), "{caption}");
    // the diagonal and the upper triangle are empty
    assert_eq!(table.rows[0][2], Cell::Empty);
    assert_eq!(table.rows[2][3], Cell::Empty);
    assert!(!matches!(table.rows[2][2], Cell::Empty));
    let tables = run(
        &mut survey(),
        "COMPUTE twice = age * 2 + 1.\nCORRELATIONS /VARIABLES=age WITH twice /PRINT=NOSIG.",
    );
    assert_eq!(text(&tables[0].rows[0][2]), "1.000**");
    assert!(
        tables[0]
            .caption
            .as_deref()
            .unwrap()
            .starts_with("* significant")
    );
}

#[test]
fn string_variables_are_refused() {
    let (commands, _) = parse("T-TEST /TESTVAL=1 /VARIABLES=plan.");
    let error = survey().execute(&commands[0].command).unwrap_err();
    assert!(error.0.contains("plan is a string variable"), "{error}");
    let (commands, _) = parse("ONEWAY age BY plan.");
    assert!(survey().execute(&commands[0].command).is_err());
    let (commands, _) = parse("CORRELATIONS age plan.");
    assert!(survey().execute(&commands[0].command).is_err());
}
