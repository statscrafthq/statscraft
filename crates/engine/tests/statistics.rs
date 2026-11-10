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
