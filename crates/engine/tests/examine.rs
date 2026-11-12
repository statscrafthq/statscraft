//! EXAMINE on the survey fixtures. The descriptives, percentiles, extreme values and case counts
//! agree with the reference program (`reference_procedures.rs`). The expected numbers here are
//! for what that program does not compute as the Algorithms manual says (the confidence interval
//! at another level, the M-estimators, Kolmogorov-Smirnov with the Lilliefors significance); they
//! come from an independent implementation of the manual's formulas in a general statistics
//! library, which read the published table of Molin and Abdi from its source document.
//! Shapiro-Wilk follows Royston's algorithm (AS R94); its values here are ours, which the
//! reference program confirms to ten decimals in `reference_procedures.rs` and SciPy to its
//! single precision.

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
    // these tests find rows and tables by variable name; table_labels.rs covers the labels
    run(&mut engine, "SET TVARS=NAMES OVARS=NAMES.");
    engine
}

/// The tables with this title, in order.
fn all<'t>(tables: &'t [Table], title: &str) -> Vec<&'t Table> {
    tables.iter().filter(|t| t.title == title).collect()
}

/// The cell of the row whose labels are `labels` (empty label cells skipped) in the column
/// whose full title is `column`.
fn cell<'t>(table: &'t Table, labels: &[&str], column: &str) -> &'t Cell {
    let index = table
        .column_titles()
        .iter()
        .position(|t| t == column)
        .unwrap_or_else(|| panic!("{}: no column {column}", table.title));
    let row = table
        .rows
        .iter()
        .find(|row| {
            let found: Vec<&str> = row[..table.row_headers]
                .iter()
                .filter_map(|c| match c {
                    Cell::Text(t) if !t.is_empty() => Some(t.as_str()),
                    _ => None,
                })
                .collect();
            found == labels
        })
        .unwrap_or_else(|| panic!("{}: no row {labels:?}", table.title));
    &row[index]
}

/// `actual` within 1e-9 (relative) of `expected`.
fn close(actual: &Cell, expected: f64) {
    let Cell::Number(actual) = actual else {
        panic!("not a number: {actual:?}, expected {expected}");
    };
    assert!(
        (actual - expected).abs() <= 1e-9 * expected.abs().max(1.0),
        "{actual} is not {expected}"
    );
}

fn text(cell: &Cell) -> &str {
    match cell {
        Cell::Text(t) => t,
        other => panic!("not text: {other:?}"),
    }
}

const NORMALITY: [&str; 6] = [
    "Kolmogorov-Smirnov / Statistic",
    "Kolmogorov-Smirnov / df",
    "Kolmogorov-Smirnov / Sig.",
    "Shapiro-Wilk / Statistic",
    "Shapiro-Wilk / df",
    "Shapiro-Wilk / Sig.",
];

const ESTIMATORS: [&str; 4] = [
    "Huber's M-Estimator",
    "Tukey's Biweight",
    "Hampel's M-Estimator",
    "Andrews' Wave",
];

#[test]
fn examine_interval_robust_estimates_and_normality() {
    let mut engine = loaded("survey_basic.sav");
    let tables = run(
        &mut engine,
        "SELECT IF resp_id <= 1030.\n\
         EXAMINE age monthly_spend /MISSING=PAIRWISE /CINTERVAL 90 /MESTIMATORS /PLOT=NPPLOT.",
    );
    let titles: Vec<&str> = tables.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "Case Processing Summary",
            "Descriptives",
            "M-Estimators",
            "Tests of Normality"
        ]
    );
    let descriptives = &tables[1];
    let interval = "90% Confidence Interval for Mean";
    for (name, mean, low, high, trimmed, median, iqr) in [
        (
            "age",
            53.2,
            46.713064973131985,
            59.68693502686802,
            53.407407407407405,
            52.0,
            36.5,
        ),
        (
            "monthly_spend",
            156.25500000000002,
            124.19893625579131,
            188.31106374420875,
            152.94436507936507,
            144.14999999999998,
            124.51500000000001,
        ),
    ] {
        let stat = |labels: &[&str]| {
            let mut all = vec![name];
            all.extend_from_slice(labels);
            cell(descriptives, &all, "Statistic").clone()
        };
        close(&stat(&["Mean"]), mean);
        close(&stat(&[interval, "Lower Bound"]), low);
        close(&stat(&[interval, "Upper Bound"]), high);
        close(&stat(&["5% Trimmed Mean"]), trimmed);
        close(&stat(&["Median"]), median);
        close(&stat(&["Interquartile Range"]), iqr);
    }
    let robust = &tables[2];
    let expected = [
        (
            "age",
            [
                54.20397400963469,
                53.78998182225012,
                53.57587715490321,
                53.787494791024024,
            ],
        ),
        (
            "monthly_spend",
            [
                144.649701390726,
                139.28416366226233,
                146.17732496776736,
                139.12723398176664,
            ],
        ),
    ];
    for (name, values) in expected {
        for (column, value) in ESTIMATORS.iter().zip(values) {
            close(cell(robust, &[name], column), value);
        }
    }
    let normality = &tables[3];
    let row = |name: &str| NORMALITY.map(|c| cell(normality, &[name], c).clone());
    let age = row("age");
    close(&age[0], 0.11202064920319454);
    close(&age[1], 30.0);
    assert_eq!(text(&age[2]), ">0.200");
    close(&age[3], 0.939703856064351);
    close(&age[4], 30.0);
    close(&age[5], 0.08930372374887008);
    let spend = row("monthly_spend");
    close(&spend[0], 0.12970452896705892);
    close(&spend[1], 28.0);
    assert_eq!(text(&spend[2]), ">0.200");
    close(&spend[3], 0.9490669841087788);
    close(&spend[5], 0.1877046750694561);
    assert!(engine.warnings().is_empty(), "{:?}", engine.warnings());
    // the normal and detrended Q-Q plot of each variable
    assert_eq!(engine.take_charts().len(), 4);
}

#[test]
fn examine_weighted_robust_estimates_and_normality() {
    let mut engine = loaded("survey_basic_phpwriter.sav");
    let tables = run(
        &mut engine,
        "SELECT IF resp_id <= 1030.\n\
         EXAMINE age /CINTERVAL 90 /MESTIMATORS /PLOT=NPPLOT.",
    );
    let descriptives = &all(&tables, "Descriptives")[0];
    let interval = "90% Confidence Interval for Mean";
    close(
        cell(descriptives, &["age", "Mean"], "Statistic"),
        55.03351731129353,
    );
    close(
        cell(descriptives, &["age", interval, "Lower Bound"], "Statistic"),
        49.76224099359086,
    );
    close(
        cell(descriptives, &["age", interval, "Upper Bound"], "Statistic"),
        60.3047936289962,
    );
    close(
        cell(descriptives, &["age", "5% Trimmed Mean"], "Statistic"),
        55.382999038913304,
    );
    close(
        cell(descriptives, &["age", "Median"], "Statistic"),
        58.66949999999999,
    );
    close(
        cell(descriptives, &["age", "Interquartile Range"], "Statistic"),
        36.15574999999998,
    );
    let robust = all(&tables, "M-Estimators")[0];
    let values = [
        56.88759123253397,
        56.356569978492274,
        56.044413384220505,
        56.34975143142801,
    ];
    for (column, value) in ESTIMATORS.iter().zip(values) {
        close(cell(robust, &["age"], column), value);
    }
    let normality = all(&tables, "Tests of Normality")[0];
    let age = NORMALITY.map(|c| cell(normality, &["age"], c).clone());
    close(&age[0], 0.13040257362712904);
    close(&age[1], 42.6138);
    close(&age[2], 0.06624611500901828);
    // the weights are rounded to whole numbers and the series expanded
    close(&age[3], 0.9248927348671178);
    close(&age[4], 43.0);
    close(&age[5], 0.0078014985613992305);
}

#[test]
fn examine_normality_of_large_samples() {
    let mut engine = loaded("survey_basic.sav");
    let tables = run(&mut engine, "EXAMINE age /STATISTICS=NONE /PLOT=NPPLOT.");
    let normality = all(&tables, "Tests of Normality")[0];
    let age = NORMALITY.map(|c| cell(normality, &["age"], c).clone());
    close(&age[0], 0.08496926166739543);
    close(&age[2], 0.0012973263191633254);
    close(&age[3], 0.9517290887761443);
    close(&age[4], 200.0);
    let Cell::Number(p) = age[5] else { panic!() };
    assert!((p - 2.7796452849009893e-06).abs() < 1e-17);
    // non-integer weights allow Shapiro-Wilk up to 50 cases only
    let mut engine = loaded("survey_basic_phpwriter.sav");
    let tables = run(
        &mut engine,
        "EXAMINE monthly_spend /STATISTICS=NONE /PLOT=NPPLOT.",
    );
    let normality = all(&tables, "Tests of Normality")[0];
    let spend = NORMALITY.map(|c| cell(normality, &["monthly_spend"], c).clone());
    close(&spend[0], 0.10262919397365305);
    close(&spend[1], 233.822);
    close(&spend[2], 2.95097464515997e-06);
    assert_eq!((&spend[3], &spend[5]), (&Cell::Empty, &Cell::Empty));
    close(&spend[4], 237.0);
}

#[test]
fn examine_shapiro_wilk_published_example() {
    // the weights of eleven men from Shapiro and Wilk (1965), who give W = 0.79 by their exact
    // coefficients (0.786 by the approximation in the Algorithms manual); Royston's gives 0.789
    let mut engine = loaded("survey_basic.sav");
    let tables = run(
        &mut engine,
        "SELECT IF resp_id <= 1011.\n\
         COMPUTE x = resp_id.\n\
         RECODE x (1001=148) (1002=154) (1003=158) (1004=160) (1005=161) (1006=162) (1007=166)\n\
         (1008=170) (1009=182) (1010=195) (1011=236).\n\
         EXAMINE x /MESTIMATORS /PLOT=NPPLOT.",
    );
    let normality = all(&tables, "Tests of Normality")[0];
    let x = NORMALITY.map(|c| cell(normality, &["x"], c).clone());
    close(&x[0], 0.25921535714887756);
    close(&x[2], 0.037407621800030794);
    close(&x[3], 0.7888146948353868);
    close(&x[5], 0.006703814056502814);
    let descriptives = all(&tables, "Descriptives")[0];
    close(
        cell(descriptives, &["x", "5% Trimmed Mean"], "Statistic"),
        169.77777777777777,
    );
    let robust = all(&tables, "M-Estimators")[0];
    let values = [
        164.52529691556424,
        161.81986672302176,
        163.21255217529924,
        161.7016228835324,
    ];
    for (column, value) in ESTIMATORS.iter().zip(values) {
        close(cell(robust, &["x"], column), value);
    }
}

#[test]
fn examine_shapiro_wilk_of_three_to_five_cases() {
    // the ages 49 59 75, then 36 and 68 added; SciPy's single precision gives the same to 1e-6,
    // and 3 cases have the exact W = (75 - 49)^2 / 2 / 344
    for (last, w, p) in [
        (1003, 338.0 / 344.0, 0.7470303510341707),
        (1004, 0.9970488548278809, 0.9900332093238831),
        (1005, 0.9763565839144167, 0.9142531247124788),
    ] {
        let mut engine = loaded("survey_basic.sav");
        let tables = run(
            &mut engine,
            &format!("SELECT IF resp_id <= {last}.\nEXAMINE age /STATISTICS=NONE /PLOT=NPPLOT."),
        );
        let normality = all(&tables, "Tests of Normality")[0];
        let tolerance = if last == 1004 { 1e-6 } else { 1e-12 };
        for (column, expected) in [(NORMALITY[3], w), (NORMALITY[5], p)] {
            let Cell::Number(got) = cell(normality, &["age"], column) else {
                panic!()
            };
            assert!(
                (got - expected).abs() < tolerance,
                "{last}: {got} vs {expected}"
            );
        }
    }
}

#[test]
fn examine_models_cells_and_labels() {
    let mut engine = loaded("survey_basic.sav");
    let tables = run(
        &mut engine,
        "EXAMINE age BY gender, gender, region BY gender /STATISTICS=EXTREME(2) /ID=plan /CINTERVAL 101.",
    );
    // the total, gender (once) and region by gender
    let summaries = all(&tables, "Case Processing Summary");
    assert_eq!(summaries.len(), 3);
    assert_eq!(summaries[0].row_headers, 1);
    assert_eq!(summaries[1].columns[1], "gender");
    assert_eq!(summaries[2].columns[1..3], ["region", "gender"]);
    close(
        cell(summaries[1], &["age", "Male"], "Cases / Valid / N"),
        106.0,
    );
    assert!(all(&tables, "Descriptives").is_empty());
    let extremes = all(&tables, "Extreme Values");
    assert_eq!(extremes[0].columns[3], "plan");
    // the ID variable is a string: its value labels each extreme case
    assert!(matches!(&extremes[0].rows[0][3], Cell::Text(t) if !t.is_empty()));
    assert_eq!(extremes[0].rows.len(), 4);
    assert_eq!(
        engine.warnings(),
        ["CINTERVAL 101 lies outside 50 to 99.99, so the 95% interval is shown"]
    );
    let tables = run(
        &mut engine,
        "EXAMINE age BY gender /NOTOTAL /STATISTICS=NONE.",
    );
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].row_headers, 2);
}

#[test]
fn examine_missing_values() {
    let mut engine = loaded("survey_basic.sav");
    // listwise: a case missing on monthly_spend counts as missing for age too
    let tables = run(&mut engine, "EXAMINE age monthly_spend /STATISTICS=NONE.");
    close(cell(&tables[0], &["age"], "Cases / Valid / N"), 181.0);
    close(cell(&tables[0], &["age"], "Cases / Missing / N"), 19.0);
    let tables = run(
        &mut engine,
        "EXAMINE age monthly_spend /MISSING=PAIRWISE /STATISTICS=NONE.",
    );
    close(cell(&tables[0], &["age"], "Cases / Valid / N"), 200.0);
    // REPORT makes the system-missing value of a factor a category of its own
    let tables = run(
        &mut engine,
        "COMPUTE f = gender.\nIF (resp_id <= 1003) f = $SYSMIS.\nEXAMINE age BY f /MISSING=REPORT /NOTOTAL.",
    );
    close(
        cell(&tables[0], &["age", "System Missing"], "Cases / Valid / N"),
        3.0,
    );
    let tables = run(&mut engine, "EXAMINE age BY f /NOTOTAL /STATISTICS=NONE.");
    assert_eq!(tables[0].rows.len(), 3);
}

#[test]
fn examine_refuses_a_string_dependent() {
    let mut engine = loaded("survey_basic.sav");
    let (commands, _) = parse("EXAMINE plan.");
    let error = engine.execute(&commands[0].command).unwrap_err();
    assert!(error.0.contains("plan is a string variable"));
}
