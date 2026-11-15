//! HILOGLINEAR and GENLOG on an invented library survey: 248 members by preferred format
//! (print or ebook), age group (three) and visit frequency (two), entered as weighted cells.
//! Every expected number was computed independently: the hierarchical fits by a published
//! iterative proportional fitting routine run to a tolerance of 1e-12, the GENLOG fits as
//! Poisson regressions with the last category as reference, the saturated parameters, the
//! generalized residual, log-odds ratio and the dispersion measures from those fits by hand.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const LIBRARY: &str = "DATA LIST LIST /format age freq (3F1) count (F3).
BEGIN DATA
1 1 1 23
1 1 2 15
1 2 1 31
1 2 2 12
1 3 1 40
1 3 2 9
2 1 1 34
2 1 2 28
2 2 1 22
2 2 2 17
2 3 1 11
2 3 2 6
END DATA.
VALUE LABELS format 1 'print' 2 'ebook'.
WEIGHT BY count.
";

/// A square table of a rating before and after a change; the corners (1, 3) and (3, 1) are
/// treated as impossible in the tests.
const RATINGS: &str = "DATA LIST LIST /before after (2F1) count (F3).
BEGIN DATA
1 1 30
1 2 8
1 3 4
2 1 6
2 2 25
2 3 9
3 1 3
3 2 7
3 3 28
END DATA.
WEIGHT BY count.
";

struct Run {
    tables: Vec<Table>,
    warnings: Vec<String>,
    engine: Engine,
}

fn run(data: &str, source: &str) -> Run {
    let (commands, errors) = parse(&format!("{data}{source}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        tables = engine
            .execute(&command.command)
            .unwrap_or_else(|e| panic!("line {}: {e}", command.first_line));
    }
    let warnings = engine.warnings().to_vec();
    Run {
        tables,
        warnings,
        engine,
    }
}

fn find<'a>(tables: &'a [Table], title: &str) -> &'a Table {
    tables.iter().find(|t| t.title == title).unwrap_or_else(|| {
        panic!(
            "no table {title}: {:?}",
            tables.iter().map(|t| &t.title).collect::<Vec<_>>()
        )
    })
}

fn num(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        Cell::Empty => f64::NAN,
        other => panic!("{other:?}"),
    }
}

/// The numbers of the row whose leading headers are `labels`.
fn row(table: &Table, labels: &[&str]) -> Vec<f64> {
    let found = table
        .rows
        .iter()
        .find(|r| {
            labels
                .iter()
                .zip(r.iter())
                .all(|(l, c)| matches!(c, Cell::Text(t) if t == l))
        })
        .unwrap_or_else(|| panic!("no row {labels:?} in {}", table.title));
    found[table.row_headers..].iter().map(num).collect()
}

/// The number in the row `labels` under column `column`.
fn at(table: &Table, labels: &[&str], column: &str) -> f64 {
    let c = table
        .columns
        .iter()
        .position(|t| t == column)
        .unwrap_or_else(|| panic!("no column {column} in {}", table.title));
    row(table, labels)[c - table.row_headers]
}

fn close(got: f64, want: f64, tolerance: f64) {
    assert!(
        (got - want).abs() <= tolerance * want.abs().max(1.0),
        "{got} against {want}"
    );
}

const TIGHT: &str = "/CRITERIA=CONVERGE(1E-10) ITERATE(500)";

#[test]
fn hiloglinear_unsaturated_model_fits_its_margins() {
    let r = run(
        LIBRARY,
        &format!("HILOGLINEAR format (1,2) age (1,3) freq (1,2) {TIGHT} /DESIGN=format*age freq."),
    );
    let fit = find(&r.tables, "Goodness-of-Fit Tests");
    let lr = row(fit, &["Likelihood Ratio"]);
    close(lr[0], 11.8760712633, 1e-8);
    assert_eq!(lr[1], 5.0);
    close(lr[2], 0.0365, 0.05);
    close(row(fit, &["Pearson"])[0], 11.3104417282, 1e-8);
    let cells = find(&r.tables, "Cell Counts and Residuals");
    close(at(cells, &["print", "3", "1"], "Count"), 40.0, 1e-12);
    // the first Count column is observed; the expected count comes two columns later
    let values = row(cells, &["print", "3", "1"]);
    close(values[2], 31.81048387097, 1e-9);
    close(values[4], 40.0 - 31.81048387097, 1e-8);
    close(
        values[5],
        (40.0 - 31.81048387097) / 31.81048387097_f64.sqrt(),
        1e-8,
    );
    let data = find(&r.tables, "Data Information");
    assert_eq!(row(data, &["", "Weighted valid"]), vec![248.0]);
    // no parameter estimates for an unsaturated model, and no warning about DESIGN
    assert!(r.tables.iter().all(|t| t.title != "Parameter Estimates"));
    assert!(r.warnings.is_empty(), "{:?}", r.warnings);
}

#[test]
fn hiloglinear_saturated_model_tests_orders_associations_and_parameters() {
    let r = run(
        LIBRARY,
        &format!("HILOGLINEAR format (1,2) age (1,3) freq (1,2) {TIGHT}."),
    );
    assert!(
        r.warnings
            .iter()
            .any(|w| w.contains("does not end with DESIGN"))
    );
    let k = find(&r.tables, "K-Way and Higher-Order Effects");
    let first = row(k, &["K-way and higher-order effects"]);
    assert_eq!(first[0], 11.0);
    close(first[1], 63.5185446787, 1e-8);
    close(first[3], 62.1612903226, 1e-8);
    let higher: Vec<Vec<f64>> = k.rows[..3]
        .iter()
        .map(|r| r[2..].iter().map(num).collect())
        .collect();
    close(higher[1][1], 33.4956543176, 1e-8);
    assert_eq!(higher[1][0], 7.0);
    close(higher[2][1], 0.935357061240, 1e-8);
    assert_eq!(higher[2][0], 2.0);
    let effects: Vec<Vec<f64>> = k.rows[3..]
        .iter()
        .map(|r| r[2..].iter().map(num).collect())
        .collect();
    assert_eq!(effects[0][0], 4.0);
    close(effects[0][1], 63.5185446787 - 33.4956543176, 1e-8);
    assert_eq!(effects[1][0], 5.0);
    close(effects[1][1], 33.4956543176 - 0.935357061240, 1e-8);
    let partial = find(&r.tables, "Partial Associations");
    for (effect, df, chi) in [
        ("format*age", 2.0, 18.5696813900),
        ("format*freq", 1.0, 3.51781794208),
        ("age*freq", 2.0, 4.37299459558),
        ("format", 1.0, 0.5808719523),
        ("age", 2.0, 7.0214567780),
        ("freq", 1.0, 22.4205616308),
    ] {
        let values = row(partial, &[effect]);
        assert_eq!(values[0], df, "{effect}");
        close(values[1], chi, 1e-7);
    }
    assert_eq!(partial.rows.len(), 6);
    let estimates = find(&r.tables, "Parameter Estimates");
    let three = row(estimates, &["format*age*freq"]);
    close(three[0], -0.0918499557797, 1e-10);
    close(three[1], 0.0938623664788, 1e-10);
    close(three[2], -0.0918499557797 / 0.0938623664788, 1e-9);
    let freq = row(estimates, &["freq"]);
    close(freq[0], 0.3169453331471, 1e-10);
    close(freq[1], 0.0724201222758, 1e-10);
    // the second parameter of format*age is format 1, age 2
    let at_two = estimates
        .rows
        .iter()
        .position(|r| matches!(&r[0], Cell::Text(t) if t == "format*age"))
        .unwrap()
        + 1;
    close(num(&estimates.rows[at_two][2]), -0.0537851328510, 1e-10);
    close(num(&estimates.rows[at_two][3]), 0.0984333203488, 1e-10);
    // the saturated model shows the counts plus DELTA and fits them exactly
    let cells = find(&r.tables, "Cell Counts and Residuals");
    let values = row(cells, &["ebook", "3", "2"]);
    close(values[0], 6.5, 1e-12);
    close(values[2], 6.5, 1e-9);
    assert!(cells.caption.as_deref().unwrap().contains("0.5"));
    let fit = row(
        find(&r.tables, "Goodness-of-Fit Tests"),
        &["Likelihood Ratio"],
    );
    assert!(fit[0].abs() < 1e-8 && fit[1] == 0.0 && fit[2].is_nan());
}

#[test]
fn hiloglinear_backward_elimination_stops_at_two_associations() {
    let r = run(
        LIBRARY,
        &format!("HILOGLINEAR format (1,2) age (1,3) freq (1,2) {TIGHT} /METHOD=BACKWARD /DESIGN."),
    );
    let steps = find(&r.tables, "Step Summary");
    let generating: Vec<String> = steps
        .rows
        .iter()
        .filter(|r| matches!(&r[1], Cell::Text(t) if t == "Generating class"))
        .map(|r| match &r[2] {
            Cell::Text(t) => t.clone(),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        generating,
        vec![
            "format*age*freq",
            "format*age, format*freq, age*freq",
            "format*age, format*freq"
        ]
    );
    let deleted = row(steps, &["", "Deleted effect 3", "age*freq"]);
    close(deleted[0], 4.37299459558, 1e-7);
    assert_eq!(deleted[1], 2.0);
    close(deleted[2], 0.1123094, 1e-5);
    let last = row(steps, &["2", "Generating class"]);
    close(last[0], 5.308352, 1e-6);
    assert_eq!(last[1], 4.0);
    // the last step's candidates: format*age and format*freq both stay
    let last_rows = &steps.rows[steps.rows.len() - 2..];
    assert!(matches!(&last_rows[1][2], Cell::Text(t) if t == "format*freq"));
    close(num(&last_rows[1][3]), 11.87607 - 5.308352, 1e-5);
    assert!(num(&last_rows[1][5]) < 0.05 && num(&last_rows[0][5]) < 0.05);
    let fit = row(
        find(&r.tables, "Goodness-of-Fit Tests"),
        &["Likelihood Ratio"],
    );
    close(fit[0], 5.308352, 1e-6);
}

#[test]
fn hiloglinear_structural_zeros_adjust_the_df() {
    let r = run(
        RATINGS,
        &format!(
            "HILOGLINEAR before after (1,3) /CWEIGHT=(2*1 0 3*1 0 2*1) {TIGHT} /PRINT=FREQ /PLOT=ALL /DESIGN=before after."
        ),
    );
    let fit = find(&r.tables, "Goodness-of-Fit Tests");
    let lr = row(fit, &["Likelihood Ratio"]);
    close(lr[0], 38.0385447823, 1e-8);
    assert_eq!(lr[1], 2.0);
    close(row(fit, &["Pearson"])[0], 41.2971754005, 1e-8);
    assert!(fit.caption.as_deref().unwrap().contains("unadjusted df 4"));
    let cells = find(&r.tables, "Cell Counts and Residuals");
    close(row(cells, &["1", "2"])[2], 16.0192237693, 1e-8);
    assert_eq!(row(cells, &["1", "3"])[2], 0.0);
    assert!(r.warnings.iter().any(|w| w.contains("cell weight of 0")));
    let mut engine = r.engine;
    let charts = engine.take_charts();
    assert_eq!(charts.len(), 3, "two residual plots and a normal plot");
}

#[test]
fn hiloglinear_cases_outside_the_range_are_counted_apart() {
    let r = run(
        LIBRARY,
        "HILOGLINEAR format (1,1) age freq (1,2) /DESIGN=format age freq.",
    );
    let data = find(&r.tables, "Data Information");
    assert_eq!(row(data, &["Cases", "Valid"]), vec![4.0]);
    assert_eq!(row(data, &["", "Out of range"]), vec![8.0]);
    assert_eq!(row(data, &["", "Weighted valid"]), vec![81.0]);
}

#[test]
fn genlog_poisson_model_with_dummy_coding() {
    let r = run(
        LIBRARY,
        "COMPUTE gr = (age = 3 & freq = 1) + 0.5 * (age = 2 & freq = 1).
GENLOG format age freq /GRESID=gr /CRITERIA=CONVERGE(1E-8) /PRINT=FREQ ADJRESID DEV ESTIM /PLOT=NONE
  /SAVE=PRED RESID (raw) /DESIGN=format age freq format*age.",
    );
    let fit = find(&r.tables, "Goodness-of-Fit Tests");
    let lr = row(fit, &["Likelihood Ratio"]);
    close(lr[0], 11.8760712633, 1e-7);
    assert_eq!(lr[1], 5.0);
    close(row(fit, &["Pearson Chi-Square"])[0], 11.3104417281, 1e-7);
    let est = find(&r.tables, "Parameter Estimates");
    let constant = row(est, &["Constant"]);
    close(constant[0], 1.7856927165, 1e-7);
    close(constant[1], 0.2574597526, 1e-7);
    close(row(est, &["[format = print]"])[0], 1.0586069541, 1e-7);
    let redundant = row(est, &["[format = ebook]"]);
    assert_eq!(redundant[0], 0.0);
    assert!(redundant[1].is_nan());
    close(row(est, &["[age = 2]"])[0], 0.8303483021, 1e-7);
    close(row(est, &["[freq = 1]"])[1], 0.1330617638, 1e-7);
    let inter = row(est, &["[format = print] * [age = 1]"]);
    close(inter[0], -1.5481551794, 1e-7);
    close(inter[1], 0.3488216083, 1e-7);
    close(inter[2], -4.438243339, 1e-7);
    let cells = find(&r.tables, "Cell Counts and Residuals");
    close(
        at(cells, &["print", "3", "1"], "Adjusted Residual"),
        2.73677202114,
        1e-7,
    );
    close(
        at(cells, &["print", "3", "2"], "Deviance"),
        -2.17524264842,
        1e-6,
    );
    let g = row(find(&r.tables, "Generalized Residuals"), &["gr"]);
    close(g[0], 77.5, 1e-12);
    close(g[1], 69.4637096777, 1e-7);
    close(g[3], 1.07240904064, 1e-6);
    close(g[4], 2.65152325476, 1e-7);
    // saved: the expected count of each case's cell
    let mut engine = r.engine;
    let tables = engine
        .execute(&parse("LIST VARIABLES=PRE_1 raw.").0[0].command)
        .unwrap();
    let list = &tables[0];
    let shown = |c: &Cell| match c {
        Cell::Text(t) => t.trim().parse::<f64>().unwrap(),
        other => num(other),
    };
    assert_eq!(shown(&list.rows[4][0]), 31.81);
    assert_eq!(shown(&list.rows[4][1]), 8.19);
}

#[test]
fn genlog_logit_model_with_log_odds_ratio_and_dispersion() {
    let r = run(
        LIBRARY,
        "COMPUTE odds = ((format = freq) * 2 - 1) / 3.
GENLOG freq BY format age /MODEL=MULTINOMIAL /GLOR=odds /CRITERIA=CONVERGE(1E-8) /PRINT=FREQ ZRESID ADJRESID ESTIM
  /DESIGN=freq freq*format freq*age.",
    );
    let fit = row(
        find(&r.tables, "Goodness-of-Fit Tests"),
        &["Likelihood Ratio"],
    );
    close(fit[0], 0.93535706124, 1e-7);
    assert_eq!(fit[1], 2.0);
    let est = find(&r.tables, "Parameter Estimates");
    assert!(
        est.rows
            .iter()
            .all(|r| !matches!(&r[0], Cell::Text(t) if t == "Constant"))
    );
    close(row(est, &["[freq = 1]"])[0], 0.8470502260, 1e-7);
    let fq = row(est, &["[freq = 1] * [format = print]"]);
    close(fq[0], 0.5266494017, 1e-7);
    close(fq[1], 0.2814995651, 1e-7);
    close(
        row(est, &["[freq = 1] * [age = 2]"])[0],
        -0.5101679751,
        1e-7,
    );
    let cells = find(&r.tables, "Cell Counts and Residuals");
    close(
        at(cells, &["1", "print", "1"], "Adjusted Residual"),
        -0.9374288792,
        1e-6,
    );
    close(
        at(cells, &["1", "print", "1"], "Standardized Residual"),
        -0.5617441676,
        1e-6,
    );
    let glor = row(find(&r.tables, "Generalized Log-Odds Ratios"), &["odds"]);
    close(glor[0], 0.526649401715, 1e-7);
    close(glor[1], 3.50015868092, 1e-7);
    let dispersion = find(&r.tables, "Analysis of Dispersion");
    let model = row(dispersion, &["Due to model"]);
    close(model[0], 5.470357101, 1e-6);
    close(model[1], 4.861725659, 1e-6);
    assert_eq!(model[2], 3.0);
    assert_eq!(row(dispersion, &["Total"])[2], 247.0);
    let measures = &find(&r.tables, "Measures of Association").rows[0];
    close(num(&measures[0]), 0.034042875181, 1e-6);
    close(num(&measures[1]), 0.043039478954, 1e-6);
    let mut engine = r.engine;
    assert_eq!(engine.take_charts().len(), 4);
}

#[test]
fn genlog_saturated_default_and_structural_zeros() {
    let r = run(LIBRARY, "GENLOG format age freq /PLOT=NONE.");
    let fit = row(
        find(&r.tables, "Goodness-of-Fit Tests"),
        &["Likelihood Ratio"],
    );
    assert!(fit[0].abs() < 1e-6 && fit[1] == 0.0);
    let cells = find(&r.tables, "Cell Counts and Residuals");
    let values = row(cells, &["ebook", "3", "2"]);
    close(values[0], 6.5, 1e-12);
    close(values[2], 6.5, 1e-6);
    let r = run(
        RATINGS,
        "COMPUTE cw = ABS(before - after) < 2.
GENLOG before after /CSTRUCTURE=cw /DESIGN=before after.",
    );
    let fit = find(&r.tables, "Goodness-of-Fit Tests");
    let lr = row(fit, &["Likelihood Ratio"]);
    close(lr[0], 38.0385447823, 1e-6);
    assert_eq!(lr[1], 2.0);
    let data = find(&r.tables, "Data Information");
    assert_eq!(row(data, &["", "Structural zeros"]), vec![2.0]);
    let cells = find(&r.tables, "Cell Counts and Residuals");
    close(row(cells, &["3", "2"])[2], 13.7637993472, 1e-6);
    assert!(row(cells, &["1", "3"])[2].is_nan());
}

#[test]
fn genlog_errors() {
    let (commands, _) = parse(&format!(
        "{LIBRARY}GENLOG format WITH age.\nGENLOG format age /DESIGN=format(3).\n"
    ));
    let mut engine = Engine::default();
    let mut errors = Vec::new();
    for c in &commands {
        if let Err(e) = engine.execute(&c.command) {
            errors.push(e.to_string());
        }
    }
    assert_eq!(errors.len(), 2, "{errors:?}");
    assert!(errors[0].contains("needs DESIGN"));
    assert!(errors[1].contains("category 3"));
}

#[test]
fn hiloglinear_maxorder_limits_the_terms() {
    let r = run(
        LIBRARY,
        &format!(
            "HILOGLINEAR format (1,2) age (1,3) freq (1,2) {TIGHT} /MAXORDER=2 /DESIGN /MAXORDER=1 /DESIGN=format*age*freq."
        ),
    );
    let fits: Vec<&Table> = r
        .tables
        .iter()
        .filter(|t| t.title == "Goodness-of-Fit Tests")
        .collect();
    assert_eq!(fits.len(), 2);
    close(row(fits[0], &["Likelihood Ratio"])[0], 0.935357061240, 1e-8);
    close(row(fits[1], &["Likelihood Ratio"])[0], 33.4956543176, 1e-8);
    assert_eq!(fits[1].caption.as_deref(), Some("Design 2"));
    assert!(
        r.tables
            .iter()
            .all(|t| t.title != "K-Way and Higher-Order Effects")
    );
}

#[test]
fn genlog_cell_covariate_gives_a_linear_effect() {
    let r = run(
        LIBRARY,
        "COMPUTE x = age.
GENLOG age WITH x /CRITERIA=CONVERGE(1E-8) /PRINT=ESTIM FREQ /DESIGN=x.",
    );
    let est = find(&r.tables, "Parameter Estimates");
    let x = row(est, &["x"]);
    close(x[0], -0.207112227593, 1e-7);
    close(x[1], 0.0786058829627, 1e-7);
    close(row(est, &["Constant"])[0], 4.814793218380, 1e-7);
    let fit = row(
        find(&r.tables, "Goodness-of-Fit Tests"),
        &["Likelihood Ratio"],
    );
    close(fit[0], 0.00468167548929, 1e-6);
    assert_eq!(fit[1], 1.0);
    close(
        row(find(&r.tables, "Cell Counts and Residuals"), &["2"])[2],
        81.4972080981,
        1e-7,
    );
}
