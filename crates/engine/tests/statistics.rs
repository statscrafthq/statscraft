//! MEANS, T-TEST, ONEWAY, CORRELATIONS and NONPAR CORR on the survey fixtures. Expected numbers come from two
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
            "Multiple Comparisons: age",
            "Homogeneous Subsets: age"
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

// GT2, Gabriel and T3: the manual's formulas in R, the studentized maximum modulus by
// integrate() to 1e-13; Dunnett's C with R's ptukey inverted to 1e-13.
#[test]
fn oneway_maximum_modulus_and_dunnett_c_comparisons() {
    let tables = run(
        &mut survey(),
        "ONEWAY age BY region /POSTHOC=GT2 GABRIEL T3 C.",
    );
    let table = titled(&tables, "Multiple Comparisons");
    assert_eq!(table.rows.len(), 80);
    // (row within a block of 20, mean difference, pooled and separate standard errors)
    let pairs = [
        (0, -8.04630788485607, 4.28334077681653, 4.15341678705107),
        (2, -11.7756497948016, 4.36616538250091, 4.38888978583132),
        (5, 4.3404255319149, 4.44594731510282, 4.45412529311948),
        (10, -8.06976744186046, 4.52579710832026, 4.67447480173158),
        (15, 4.93933265925177, 4.03562092665536, 4.12604527891637),
    ];
    // per test and pair: significance and half width of the interval
    let expected: [(&str, [(f64, f64); 5]); 4] = [
        (
            "Hochberg",
            [
                (0.466964353719703, 12.1223275845923),
                (0.0731224414059589, 12.3567303684205),
                (0.98070599499219, 12.5825220512973),
                (0.541767051641856, 12.8085057872127),
                (0.915934445730122, 11.4212530427036),
            ],
        ),
        (
            "Gabriel",
            [
                (0.462278637089227, 12.0829778129381),
                (0.0721835146673095, 12.3355379420137),
                (0.979835703075646, 12.5046589612592),
                (0.536066727245728, 12.7572190903347),
                (0.915864677412957, 11.419630334126),
            ],
        ),
        (
            "Dunnett T3",
            [
                (0.429670727621453, 11.9638867581345),
                (0.0848650046716329, 12.641771269444),
                (0.979361505977354, 12.8975211616958),
                (0.590068760912715, 13.5143570973154),
                (0.924100288104395, 11.8471444308118),
            ],
        ),
        (
            "Dunnett C",
            [
                (f64::NAN, 11.8933455187),
                (f64::NAN, 12.5809617259),
                (f64::NAN, 12.8268056895),
                (f64::NAN, 13.4668889672),
                (f64::NAN, 11.7440320895),
            ],
        ),
    ];
    for (t, (name, values)) in expected.iter().enumerate() {
        assert_eq!(text(&table.rows[20 * t][0]), *name);
        for ((row, diff, pooled, separate), (sig, half)) in pairs.iter().zip(values) {
            let row = &table.rows[20 * t + row];
            near(number(&row[3]), *diff, 12);
            let se = if t < 2 { pooled } else { separate };
            near(number(&row[4]), *se, 12);
            if sig.is_nan() {
                assert_eq!(row[5], Cell::Empty);
            } else {
                near(number(&row[5]), *sig, 9);
            }
            near(number(&row[6]), diff - half, 8);
            near(number(&row[7]), diff + half, 8);
        }
    }
}

// Dunnett: significances from R's multcomp (glht with a Dunnett contrast, single-step
// adjustment, good to a few units in 1e-7); critical values from the double integral over the scale
// and the shared normal done again with R's integrate() to 1e-10.
#[test]
fn oneway_dunnett_two_and_one_sided() {
    let tables = run(
        &mut survey(),
        "ONEWAY age BY region /POSTHOC=DUNNETT DUNNETTL(2) DUNNETTR(1) DUNNETT(6).",
    );
    let table = titled(&tables, "Multiple Comparisons");
    assert_eq!(table.rows.len(), 12);
    // (test, group, control, difference, standard error, significance)
    let expected = [
        (
            "Dunnett t (2-sided)",
            "North",
            "Central",
            -6.83631713555,
            4.30283928253,
            0.330592383867,
        ),
        (
            "",
            "South",
            "Central",
            1.20999074931,
            3.94586420530,
            0.994697399094,
        ),
        (
            "",
            "East",
            "Central",
            -3.13043478261,
            4.46473573794,
            0.899569520612,
        ),
        (
            "",
            "West",
            "Central",
            4.93933265925,
            4.03562092666,
            0.566466117956,
        ),
        (
            "Dunnett t (<control)",
            "North",
            "South",
            -8.04630788486,
            4.28334077682,
            0.0975640896945,
        ),
        (
            "",
            "East",
            "South",
            -4.34042553191,
            4.44594731510,
            0.3993564227007,
        ),
        (
            "",
            "West",
            "South",
            3.72934190995,
            4.01482485082,
            0.9787787427192,
        ),
        (
            "",
            "Central",
            "South",
            -1.20999074931,
            3.94586420530,
            0.7048201291919,
        ),
        (
            "Dunnett t (>control)",
            "South",
            "North",
            8.04630788486,
            4.28334077682,
            0.0908293336653,
        ),
        (
            "",
            "East",
            "North",
            3.70588235294,
            4.76560899501,
            0.4600040870144,
        ),
        (
            "",
            "West",
            "North",
            11.77564979480,
            4.36616538250,
            0.0130367779047,
        ),
        (
            "",
            "Central",
            "North",
            6.83631713555,
            4.30283928253,
            0.1551811427868,
        ),
    ];
    let critical = [2.4725107092, 2.18937619991, 2.16016121226];
    for (r, (test, group, control, diff, se, sig)) in expected.into_iter().enumerate() {
        let row = &table.rows[r];
        assert_eq!(
            [text(&row[0]), text(&row[1]), text(&row[2])],
            [test, group, control]
        );
        near(number(&row[3]), diff, 10);
        near(number(&row[4]), se, 10);
        near(number(&row[5]), sig, 6);
        let half = critical[r / 4] * se;
        match r / 4 {
            0 => {
                near(number(&row[6]), diff - half, 8);
                near(number(&row[7]), diff + half, 8);
            }
            // one-sided intervals are open on one side
            1 => {
                assert_eq!(row[6], Cell::Empty);
                near(number(&row[7]), diff + half, 8);
            }
            _ => {
                near(number(&row[6]), diff - half, 8);
                assert_eq!(row[7], Cell::Empty);
            }
        }
    }
    let mut engine = survey();
    run(&mut engine, "ONEWAY age BY region /POSTHOC=DUNNETT(6).");
    assert_eq!(
        engine.warnings(),
        ["no Dunnett t (2-sided) test for age: there are only 5 groups"]
    );
}

// Homogeneous subsets: the manual's step-down rules implemented again in R with its ptukey,
// pf and integrate (for the studentized maximum modulus). The weighted y2 gives different
// subsets for different tests.
#[test]
fn oneway_homogeneous_subsets_weighted() {
    let tables = run(
        &mut weighted(),
        "COMPUTE y2 = region * 0.25 + age / 20 + (region = 3) * 0.4.\n\
         ONEWAY y2 BY region /POSTHOC=SNK BTUKEY DUNCAN TUKEY SCHEFFE GT2 GABRIEL FREGW QREGW.",
    );
    let table = titled(&tables, "Homogeneous Subsets: y2");
    assert_eq!(table.columns[2..], ["N", "1", "2", "3"]);
    assert_eq!(table.column_groups[3], ["Subset for alpha = 0.05"]);
    assert!(table.caption.as_deref().unwrap().contains("harmonic mean"));
    // nine tests of five groups and a Sig. row each
    assert_eq!(table.rows.len(), 54);
    let mean = |row: usize, subset: usize| match &table.rows[row][3 + subset] {
        Cell::Number(n) => Some(*n),
        _ => None,
    };
    // the subsets of each test as (first, last) positions of the means in ascending order, and
    // their significances
    let expected = [
        (
            "Student-Newman-Keuls",
            [(0, 0), (1, 1), (2, 4)],
            [Some(1.0), Some(1.0), Some(0.287603880627057)],
        ),
        ("Tukey B", [(0, 0), (1, 2), (2, 4)], [None, None, None]),
        (
            "Duncan",
            [(0, 0), (1, 1), (2, 4)],
            [Some(1.0), Some(1.0), Some(0.155964385009173)],
        ),
        (
            "Tukey HSD",
            [(0, 0), (1, 2), (2, 4)],
            [Some(1.0), Some(0.160728620877921), Some(0.556506553087587)],
        ),
        (
            "Scheffe",
            [(0, 0), (1, 2), (2, 4)],
            [Some(1.0), Some(0.278492129552933), Some(0.684268631752953)],
        ),
        (
            "Hochberg",
            [(0, 0), (1, 2), (2, 4)],
            [Some(1.0), Some(0.218376700971119), Some(0.753334969738544)],
        ),
        (
            "Gabriel",
            [(0, 0), (1, 2), (2, 4)],
            [Some(1.0), Some(0.218376700971119), Some(0.753334969738544)],
        ),
        (
            "Ryan-Einot-Gabriel-Welsch F",
            [(0, 0), (1, 2), (2, 4)],
            [
                Some(1.0),
                Some(0.0846484868876268),
                Some(0.5166919922932013),
            ],
        ),
        (
            "Ryan-Einot-Gabriel-Welsch Range",
            [(0, 0), (1, 2), (2, 4)],
            [Some(1.0), Some(0.138855253393862), Some(0.588094134722658)],
        ),
    ];
    let means = [
        2.4270000360491,
        3.15641609444022,
        3.58181954576465,
        3.79355148656282,
        3.86588672600676,
    ];
    for (t, (name, sets, sigs)) in expected.iter().enumerate() {
        let first = 6 * t;
        assert_eq!(text(&table.rows[first][0]), *name);
        assert_eq!(text(&table.rows[first + 2][1]), "East");
        for (s, &(a, b)) in sets.iter().enumerate() {
            for (position, &m) in means.iter().enumerate() {
                match mean(first + position, s) {
                    Some(got) if a <= position && position <= b => near(got, m, 13),
                    None if !(a <= position && position <= b) => {}
                    other => panic!("{name} subset {s} position {position}: {other:?}"),
                }
            }
            match sigs[s] {
                // R's ptukey is good to about 1e-10
                Some(sig) => near(number(&table.rows[first + 5][3 + s]), sig, 9),
                None => assert_eq!(table.rows[first + 5][3 + s], Cell::Empty),
            }
        }
    }
    assert_eq!(text(&table.rows[5][1]), "Sig.");
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
    let (commands, _) = parse("NONPAR CORR age plan.");
    assert!(survey().execute(&commands[0].command).is_err());
}

/// Coefficient, significance and N of row variable `i` and column `j` in a NONPAR CORR block
/// that starts at row `block` (three statistic rows per variable).
fn rank_cell(table: &Table, block: usize, i: usize, j: usize) -> [Cell; 3] {
    let row = |k: usize| table.rows[block + 3 * i + k][3 + j].clone();
    [row(0), row(1), row(2)]
}

// Rank correlations: R's cor() of average ranks (rho) and with method "kendall" (tau-b);
// significances from the manual's formulas computed in R; weighted data repeated
// round(weight) times.
#[test]
fn nonpar_corr_pairwise_both_coefficients() {
    let tables = run(
        &mut survey(),
        "NONPAR CORR /VARIABLES=age monthly_spend q1_satisfaction q2_recommend /PRINT=BOTH.",
    );
    let table = &tables[0];
    assert_eq!(table.rows.len(), 24);
    assert_eq!(text(&table.rows[0][0]), "Kendall's tau_b");
    assert_eq!(text(&table.rows[12][0]), "Spearman's rho");
    assert_eq!(text(&table.rows[1][2]), "Sig. (2-tailed)");
    let expected = [
        (
            0,
            [
                0.0789618982605368,
                0.182482700978464,
                -0.0365908239847991,
                0.5018312660273193,
            ],
        ),
        (
            12,
            [
                0.106386470670086,
                0.179210463903539,
                -0.0530696300049373,
                0.4842242439449533,
            ],
        ),
    ];
    for (block, values) in expected {
        let [r, sig, n] = rank_cell(table, block, 1, 2);
        near(number(&r), values[0], 13);
        near(number(&sig), values[1], 12);
        assert_eq!(number(&n), 161.0);
        let [r, sig, n] = rank_cell(table, block, 0, 3);
        near(number(&r), values[2], 13);
        near(number(&sig), values[3], 12);
        assert_eq!(number(&n), 176.0);
        // the diagonal: 1 without a significance
        let [r, sig, _] = rank_cell(table, block, 2, 2);
        assert_eq!((number(&r), sig), (1.0, Cell::Empty));
    }
}

#[test]
fn nonpar_corr_listwise_include_and_weights() {
    let tables = run(
        &mut survey(),
        "NONPAR CORR /VARIABLES=age monthly_spend q1_satisfaction q2_recommend /PRINT=BOTH /MISSING=LISTWISE INCLUDE.",
    );
    let table = &tables[0];
    assert_eq!(table.rows.len(), 16);
    assert_eq!(table.caption.as_deref(), Some("Listwise N = 181."));
    let cell =
        |block: usize, i: usize, j: usize, k: usize| number(&table.rows[block + 2 * i + k][3 + j]);
    near(cell(0, 2, 3, 0), 0.00419133471368308, 13);
    near(cell(0, 2, 3, 1), 0.942048263476441, 12);
    near(cell(8, 2, 3, 0), 0.00508521085800225, 13);
    near(cell(8, 2, 3, 1), 0.945832625525696, 12);
    near(cell(0, 0, 2, 0), 0.0672129456920739, 13);
    near(cell(8, 0, 2, 1), 0.2113201412163491, 12);
    let tables = run(
        &mut weighted(),
        "NONPAR CORR /VARIABLES=age monthly_spend q1_satisfaction q2_recommend /PRINT=BOTH ONETAIL.",
    );
    let table = &tables[0];
    assert_eq!(text(&table.rows[1][2]), "Sig. (1-tailed)");
    let expected = [
        (
            0,
            [
                0.1136075364132749,
                0.0432193787396817,
                0.0286224732276349,
                0.5160567803509514,
            ],
        ),
        (
            12,
            [
                0.1426714876297558,
                0.0433367446076042,
                0.0447013968550983,
                0.4934238881575562,
            ],
        ),
    ];
    for (block, values) in expected {
        let [r, sig, n] = rank_cell(table, block, 2, 3);
        near(number(&r), values[0], 13);
        near(number(&sig), values[1] / 2.0, 12);
        assert_eq!(number(&n), 201.0);
        let [r, sig, n] = rank_cell(table, block, 0, 1);
        near(number(&r), values[2], 13);
        near(number(&sig), values[3] / 2.0, 12);
        assert_eq!(number(&n), 237.0);
    }
}

#[test]
fn nonpar_corr_flags_layout_and_constant_variables() {
    let tables = run(
        &mut survey(),
        "COMPUTE twice = age * 2 + 1.\nCOMPUTE same = 4.\n\
         NONPAR CORR /VARIABLES=age WITH twice same /PRINT=KENDALL NOSIG\n\
         /age monthly_spend q1_satisfaction /PRINT=LNODIAG.",
    );
    assert_eq!(tables.len(), 2);
    let table = &tables[0];
    assert_eq!(table.columns[3..], ["twice", "same"]);
    assert_eq!(text(&table.rows[0][0]), "Kendall's tau_b");
    assert_eq!(text(&table.rows[0][3]), "1.000**");
    // a constant variable has no coefficient
    assert_eq!(table.rows[0][4], Cell::Empty);
    assert_eq!(table.rows[1][4], Cell::Empty);
    assert_eq!(number(&table.rows[2][4]), 200.0);
    let caption = table.caption.as_deref().unwrap();
    assert!(caption.starts_with("* significant"), "{caption}");
    // the later PRINT keeps KENDALL: the diagonal and the upper triangle are empty
    let table = &tables[1];
    assert_eq!(table.rows.len(), 9);
    assert_eq!(table.rows[0][3], Cell::Empty);
    assert_eq!(table.rows[3][5], Cell::Empty);
    assert!(!matches!(table.rows[3][3], Cell::Empty));
}
