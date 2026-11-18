//! BOOTSTRAP on invented data.
//!
//! The expected bias, standard errors and intervals come from R's boot package run as a black
//! box on the same twelve cases and the same 99 resamples: the indices were made outside
//! Statscraft from the congruential generator that SET SEED=4242 starts (u = seed / 2^31, pick
//! floor(u * 12)), passed to R, and boot's replicate values replaced by the statistic on those
//! indices. Bias is mean(t*) - t0, the standard error sd(t*), the percentile interval
//! boot.ci(type = "perc"). For BCa the acceleration comes from empinf(type = "jack") and the
//! bias correction is the one the manual gives, qnorm(#(t* <= t0) / (B + 1)); the adjusted
//! tails are read off by boot.ci's percentile interpolation. (boot.ci's own BCa counts t* < t0
//! over B and so differs slightly: 4.45277 to 5.79621 for the mean.)

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /x y g.
BEGIN DATA.
4.2 2.1 1
5.1 2.9 2
3.8 1.7 1
6.0 3.4 2
5.5 2.6 1
4.9 2.2 2
7.2 4.1 1
3.1 1.5 2
5.8 3.3 1
4.4 2.0 2
6.6 3.9 1
5.0 2.8 2
END DATA.
";

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

fn number(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        other => panic!("not a number: {other:?}"),
    }
}

fn close(got: f64, want: f64) {
    assert!((got - want).abs() < 1e-10, "got {got}, want {want}");
}

fn table<'a>(tables: &'a [Table], title: &str) -> &'a Table {
    tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"))
}

/// The four numbers right of column `col` in row `row`: bias, error, lower, upper.
fn after(t: &Table, row: usize, col: usize) -> Vec<f64> {
    t.rows[row][col + 1..col + 5].iter().map(number).collect()
}

#[test]
fn a_mean_matches_boot_in_r() {
    let mut engine = Engine::default();
    let source = format!(
        "{DATA}SET SEED=4242.
BOOTSTRAP /CRITERIA NSAMPLES=99.
DESCRIPTIVES x /STATISTICS=MEAN."
    );
    let tables = run(&mut engine, &source);
    let t = table(&tables, "Descriptive Statistics");
    let mean = t.columns.iter().position(|c| c == "Statistic").unwrap();
    assert_eq!(t.column_groups[mean], ["Mean"]);
    assert_eq!(
        t.columns[mean + 1..mean + 5],
        ["Bias", "Std. Error", "Lower", "Upper"]
    );
    assert_eq!(
        t.column_groups[mean + 3],
        ["Mean", "Bootstrap", "95% Confidence Interval"]
    );
    close(number(&t.rows[0][mean]), 5.13333333333333);
    let b = after(t, 0, mean);
    close(b[0], 0.01540404040404);
    close(b[1], 0.32013709603171);
    close(b[2], 4.45903749164733);
    close(b[3], 5.81836877923435);
    // the next procedure runs without resampling
    let tables = run(&mut engine, "DESCRIPTIVES x /STATISTICS=MEAN.");
    let t = table(&tables, "Descriptive Statistics");
    assert!(!t.columns.iter().any(|c| c == "Bias"));
}

#[test]
fn a_mean_bca_matches_boot_in_r() {
    let mut engine = Engine::default();
    let source = format!(
        "{DATA}SET SEED=4242.
BOOTSTRAP /CRITERIA CITYPE=BCA NSAMPLES=99.
DESCRIPTIVES x /STATISTICS=MEAN."
    );
    let tables = run(&mut engine, &source);
    let t = table(&tables, "Descriptive Statistics");
    let mean = t.columns.iter().position(|c| c == "Statistic").unwrap();
    assert_eq!(
        t.column_groups[mean + 3],
        ["Mean", "Bootstrap", "BCa 95% Confidence Interval"]
    );
    let b = after(t, 0, mean);
    close(b[0], 0.01540404040404);
    close(b[1], 0.32013709603171);
    close(b[2], 4.4504431455868);
    close(b[3], 5.79166666666667);
}

/// The Pearson row of x with y and its four bootstrap rows below it.
fn correlation(bca: bool) -> Vec<f64> {
    let mut engine = Engine::default();
    let citype = if bca { "BCA" } else { "PERCENTILE" };
    let source = format!(
        "{DATA}SET SEED=4242.
BOOTSTRAP /CRITERIA CITYPE={citype} NSAMPLES=99.
CORRELATIONS x y."
    );
    let tables = run(&mut engine, &source);
    let t = table(&tables, "Correlations");
    let labels: Vec<String> = (0..t.rows.len())
        .map(|r| statscraft_engine::cell_text(t, r, t.row_headers - 1))
        .collect();
    let pearson = labels
        .iter()
        .position(|l| l == "Pearson Correlation")
        .unwrap();
    assert_eq!(labels[pearson + 1], "Bootstrap Bias");
    assert!(labels[pearson + 4].ends_with("Upper"));
    let y = t.columns.iter().position(|c| c == "y").unwrap();
    (pearson..pearson + 5)
        .map(|r| number(&t.rows[r][y]))
        .collect()
}

#[test]
fn a_correlation_matches_boot_in_r() {
    let r = correlation(false);
    close(r[0], 0.96909800177698);
    close(r[1], -0.00434244618694);
    close(r[2], 0.02417617086953);
    close(r[3], 0.88655427924151);
    close(r[4], 0.99359853213688);
    let r = correlation(true);
    close(r[3], 0.8686735715517);
    close(r[4], 0.98904493825694);
}

#[test]
fn the_same_seed_gives_the_same_results() {
    let source = format!(
        "{DATA}SET SEED=77.
BOOTSTRAP /CRITERIA NSAMPLES=50.
MEANS x BY g."
    );
    let first = run(&mut Engine::default(), &source);
    let second = run(&mut Engine::default(), &source);
    let rows = |tables: &[Table]| {
        tables
            .iter()
            .find(|t| t.title.starts_with("Report"))
            .unwrap()
            .rows
            .clone()
    };
    assert_eq!(rows(&first), rows(&second));
    // the MEANS report has its bootstrap columns
    let report = first
        .iter()
        .find(|t| t.title.starts_with("Report"))
        .unwrap();
    assert!(report.columns.iter().any(|c| c == "Bias"));
}

#[test]
fn stratified_resamples_keep_the_strata() {
    // within each stratum of g the mean of a constant-per-stratum variable never moves
    let mut engine = Engine::default();
    let source = format!(
        "{DATA}COMPUTE s = g * 10.
SET SEED=5.
BOOTSTRAP /SAMPLING METHOD=STRATIFIED(STRATA=g) /CRITERIA NSAMPLES=40.
DESCRIPTIVES s /STATISTICS=MEAN STDDEV."
    );
    let tables = run(&mut engine, &source);
    let t = table(&tables, "Descriptive Statistics");
    let mean = t.columns.iter().position(|c| c == "Statistic").unwrap();
    // six cases of 10 and six of 20 in every resample: the mean stays 15
    let b = after(t, 0, mean);
    close(b[0], 0.0);
    close(b[1], 0.0);
    close(b[2], 15.0);
    close(b[3], 15.0);
}

#[test]
fn missing_values_of_the_named_variables_set_the_cases() {
    let mut engine = Engine::default();
    let source = format!(
        "{DATA}COMPUTE z = x.
IF (g = 2) z = $SYSMIS.
SET SEED=3.
BOOTSTRAP /VARIABLES INPUT=z /CRITERIA NSAMPLES=20.
DESCRIPTIVES x /STATISTICS=MEAN."
    );
    let tables = run(&mut engine, &source);
    let t = table(&tables, "Descriptive Statistics");
    let n = t.columns.iter().position(|c| c == "N").unwrap();
    // only the six cases of g = 1 take part, the original run included
    close(number(&t.rows[0][n]), 6.0);
}

#[test]
fn other_procedures_warn_and_run_as_usual() {
    let mut engine = Engine::default();
    let tables = run(&mut engine, &format!("{DATA}BOOTSTRAP.\nONEWAY x BY g."));
    assert!(!tables.is_empty());
    assert!(
        engine
            .warnings()
            .iter()
            .any(|w| w.contains("BOOTSTRAP is not applied to ONEWAY")),
        "{:?}",
        engine.warnings()
    );
}

#[test]
fn split_groups_and_weights_are_resampled_too() {
    let mut engine = Engine::default();
    let source = format!(
        "{DATA}COMPUTE w = 2.
WEIGHT BY w.
SORT CASES BY g.
SPLIT FILE LAYERED BY g.
SET SEED=9.
BOOTSTRAP /CRITERIA CITYPE=BCA NSAMPLES=30.
DESCRIPTIVES x /STATISTICS=MEAN."
    );
    let tables = run(&mut engine, &source);
    let t = table(&tables, "Descriptive Statistics");
    let n = t.columns.iter().position(|c| c == "N").unwrap();
    let mean = t.columns.iter().position(|c| c == "Statistic").unwrap();
    let rows: Vec<&Vec<Cell>> = t
        .rows
        .iter()
        .filter(|r| matches!(r[mean + 2], Cell::Number(_)))
        .collect();
    // one row per group, each with its weighted count and a positive standard error
    assert_eq!(rows.len(), 2);
    for row in rows {
        close(number(&row[n]), 12.0);
        assert!(number(&row[mean + 2]) > 0.0);
    }
}

#[test]
fn t_test_frequencies_and_regression_get_bootstrap_results() {
    let mut engine = Engine::default();
    let source = format!(
        "{DATA}SET SEED=11.
BOOTSTRAP /CRITERIA NSAMPLES=30.
T-TEST GROUPS=g(1 2) /VARIABLES=x.
BOOTSTRAP /CRITERIA NSAMPLES=30.
FREQUENCIES x /FORMAT=NOTABLE /STATISTICS=MEAN MEDIAN.
BOOTSTRAP /CRITERIA NSAMPLES=30.
REGRESSION /DEPENDENT=y /METHOD=ENTER x."
    );
    let tables = run(&mut engine, &source);
    let has_columns = |title: &str| {
        let t = tables.iter().find(|t| t.title.starts_with(title)).unwrap();
        t.columns.iter().any(|c| c == "Bias")
    };
    assert!(has_columns("Group Statistics"));
    assert!(has_columns("Independent Samples Test"));
    assert!(has_columns("Coefficients"));
    let stats = table(&tables, "Statistics");
    let labels: Vec<String> = (0..stats.rows.len())
        .map(|r| statscraft_engine::cell_text(stats, r, stats.row_headers - 1))
        .collect();
    assert_eq!(labels.iter().filter(|l| *l == "Bootstrap Bias").count(), 2);
}
