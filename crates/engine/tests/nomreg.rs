//! NOMREG on synthetic data. The expected numbers come from an independent fit of the same
//! data by a general statistics package (multinomial logit by quasi-Newton, converged tightly),
//! with its reference category set to ours and its factor contrasts set to make the last level
//! the reference; the multinomial constant, the goodness-of-fit sums and the intercept-only
//! likelihood were written out separately from the fitted probabilities.

use statscraft_data::Values;
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

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

fn run_error(source: &str) -> String {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    for command in &commands {
        if let Err(error) = engine.execute(&command.command) {
            return error.0;
        }
    }
    panic!("no error");
}

/// The row headers of every row joined with " / ", an empty header taking the one above it.
fn row_keys(table: &Table) -> Vec<String> {
    let mut carried = vec![String::new(); table.row_headers];
    table
        .rows
        .iter()
        .map(|row| {
            for (level, cell) in row.iter().take(table.row_headers).enumerate() {
                let text = match cell {
                    Cell::Text(t) => t.clone(),
                    Cell::Number(n) => n.to_string(),
                    Cell::Empty => continue,
                };
                carried[level] = text;
                for deeper in carried.iter_mut().skip(level + 1) {
                    deeper.clear();
                }
            }
            let parts: Vec<&str> = carried
                .iter()
                .map(String::as_str)
                .filter(|s| !s.is_empty())
                .collect();
            parts.join(" / ")
        })
        .collect()
}

fn cell<'t>(tables: &'t [Table], title: &str, row: &str, column: &str) -> &'t Cell {
    let table = tables
        .iter()
        .rev()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"));
    let r = row_keys(table)
        .iter()
        .position(|k| k == row)
        .unwrap_or_else(|| panic!("{title}: no row {row} in {:?}", row_keys(table)));
    let c = table
        .column_titles()
        .iter()
        .position(|t| t == column)
        .unwrap_or_else(|| panic!("{title}: no column {column} in {:?}", table.column_titles()));
    &table.rows[r][c]
}

fn value(tables: &[Table], title: &str, row: &str, column: &str) -> f64 {
    match cell(tables, title, row, column) {
        Cell::Number(n) => *n,
        other => panic!("{title} / {row} / {column}: not a number: {other:?}"),
    }
}

fn check(tables: &[Table], title: &str, row: &str, column: &str, expected: f64) {
    let actual = value(tables, title, row, column);
    assert!(
        (actual - expected).abs() <= 1e-6 * expected.abs().max(1.0),
        "{title} / {row} / {column}: {actual} != {expected}"
    );
}

fn values(engine: &Engine, name: &str) -> Vec<Option<f64>> {
    let data = engine.dataset().unwrap();
    match data.variable(name).unwrap().values() {
        Values::Numeric(v) => (0..data.case_count()).map(|k| v.get(k)).collect(),
        Values::String(_) => panic!("{name} is a string"),
    }
}

/// How 84 invented commuters get to work: mode (1 walk, 2 bike, 3 bus), district (1 to 3, 9
/// user-missing), shift, the distance in km and a case weight w. One case lacks km, one the
/// mode, one has district 9 and one a weight of 0.
const DATA: &str = "DATA LIST FREE /mode district (F1) shift (A5) km w.
BEGIN DATA.
3 1 day 5.7 1
3 1 day 6.4 1
3 3 day 13.5 1.5
3 1 night 13.4 2
1 1 day 5.1 2
3 3 night . 1
1 2 day 1.2 1
3 1 day 9.4 2
1 1 night 8.6 1
1 1 day 9.1 0.5
3 3 day 8.7 1.5
3 3 day 6.3 1
1 2 day 8.1 1.5
3 1 day 12.2 1.5
3 3 day 10.1 0.5
1 1 day 3.9 1
3 1 night 0.9 1
. 1 day 12.5 2
2 2 night 6.5 1
3 3 night 13.2 0.5
3 3 day 13.3 1
3 3 night 9.2 1
3 2 day 13.1 2
3 2 night 7.6 1
1 1 day 7.1 1
3 2 night 6.2 1
2 2 night 7.3 2
1 1 day 5.1 1
3 1 day 10.1 2
3 1 day 10.9 1.5
3 9 night 9.5 1
1 1 day 6.9 0.5
1 2 day 12.2 1.5
3 3 night 11.1 0.5
1 3 day 6.6 0.5
1 3 night 5.5 2
1 1 day 3.7 2
2 2 day 4.3 2
3 3 night 13.7 1
1 2 day 3.2 1.5
3 1 day 13.3 2
3 2 day 5.8 1
1 3 day 9.6 0.5
3 2 day 13.7 1
2 2 day 2.3 0
3 1 night 0.9 1.5
2 1 night 13.2 1
3 1 day 9.9 1
3 1 day 3.1 0.5
1 2 night 2.7 0.5
1 3 day 3.2 1.5
2 3 night 5.9 1
2 2 night 13.8 0.5
3 3 night 1.5 2
1 2 night 3.1 1
1 1 night 8.7 1.5
3 1 day 11.1 1
3 1 day 11.4 1
1 1 night 6.6 0.5
2 3 night 9.8 0.5
1 2 night 1.1 1
2 2 day 3.6 1
2 2 day 12.0 1.5
2 1 night 5.9 1.5
1 3 night 1.9 0.5
3 3 night 11.7 1.5
2 2 day 13.2 1
2 2 night 8.3 1.5
2 2 night 3.2 2
1 3 day 4.1 0.5
2 1 night 10.3 0.5
3 2 day 8.2 0.5
3 3 night 8.3 1
2 2 day 8.5 1
3 3 night 5.1 1.5
2 2 night 1.2 0.5
1 1 night 1.4 1
3 2 day 8.1 0.5
1 3 night 7.5 1
1 2 night 3.3 0.5
3 2 day 3.4 1
1 1 day 10.5 1
1 1 day 12.8 1
1 1 day 1.4 1
END DATA.
MISSING VALUES district (9).
";

const MFI: &str = "Model Fitting Information";
const PE: &str = "Parameter Estimates";
const LRT: &str = "Likelihood Ratio Tests";
const CHI: &str = "Likelihood Ratio Tests / Chi-Square";
const DF: &str = "Likelihood Ratio Tests / df";
const SIG: &str = "Likelihood Ratio Tests / Sig.";
const M2LL: &str = "Model Fitting Criteria / -2 Log Likelihood";
const REDUCED: &str = "Model Fitting Criteria / -2 Log Likelihood of Reduced Model";

#[test]
fn nomreg_main_effects_match_the_reference_fit_with_the_last_category_as_base() {
    let mut engine = Engine::default();
    let source = format!(
        "{DATA}NOMREG mode BY district WITH km /PRINT=CPS MFI PARAMETER LRT FIT SUMMARY CLASSTABLE IC\n\
         /SAVE=ESTPROB PREDCAT PCPROB ACPROB(hit)."
    );
    let t = run(&mut engine, &source);
    // the three cases missing on some variable are left out; the zero weight counts without WEIGHT
    check(&t, "Case Processing Summary", "Valid", "N", 81.0);
    check(&t, "Case Processing Summary", "Missing", "N", 3.0);
    check(&t, "Case Processing Summary", "mode / 2", "N", 17.0);
    check(&t, "Case Processing Summary", "Subpopulation", "N", 75.0);
    // -2LL with the multinomial constant 2.07944154168 of the 75 subpopulations
    check(&t, MFI, "Intercept Only", M2LL, 167.235103832);
    check(&t, MFI, "Final", M2LL, 142.367209019);
    check(&t, MFI, "Final", CHI, 24.8678948133);
    check(&t, MFI, "Final", DF, 6.0);
    check(&t, MFI, "Final", SIG, 0.000361228787352);
    check(
        &t,
        MFI,
        "Final",
        "Model Fitting Criteria / AIC",
        158.367209019,
    );
    check(
        &t,
        MFI,
        "Final",
        "Model Fitting Criteria / BIC",
        177.522802256,
    );
    check(
        &t,
        "Goodness-of-Fit",
        "Pearson",
        "Chi-Square",
        150.232928942,
    );
    check(
        &t,
        "Goodness-of-Fit",
        "Deviance",
        "Chi-Square",
        138.208325936,
    );
    check(&t, "Goodness-of-Fit", "Deviance", "df", 142.0);
    check(&t, "Goodness-of-Fit", "Pearson", "Sig.", 0.301955273837);
    check(
        &t,
        "Pseudo R-Square",
        "Cox and Snell",
        "Value",
        0.264357525837,
    );
    check(&t, "Pseudo R-Square", "Nagelkerke", "Value", 0.300582431489);
    check(&t, "Pseudo R-Square", "McFadden", "Value", 0.145091991037);
    // reduced kernels 158.253739974 and 160.171124358, less the same constant
    check(&t, LRT, "km", REDUCED, 158.253739974 - 2.0 * 2.07944154168);
    check(&t, LRT, "km", CHI, 11.7276478712);
    check(&t, LRT, "km", DF, 2.0);
    check(&t, LRT, "km", SIG, 0.00284036152998);
    check(&t, LRT, "district", CHI, 13.6450322561);
    check(&t, LRT, "district", DF, 4.0);
    check(&t, LRT, "district", SIG, 0.00851854378715);
    check(&t, PE, "1 / Intercept", "B", 1.22824445009);
    check(&t, PE, "1 / km", "B", -0.2403875092570);
    check(&t, PE, "1 / [district=1]", "B", 0.613240403266);
    check(&t, PE, "1 / [district=2]", "Std. Error", 0.745852690021);
    check(&t, PE, "2 / Intercept", "Std. Error", 1.066107992901);
    check(&t, PE, "2 / km", "Std. Error", 0.0855600614629);
    check(&t, PE, "2 / [district=2]", "B", 2.218031641361);
    check(
        &t,
        PE,
        "2 / [district=2]",
        "Wald",
        (2.218031641361f64 / 0.893264444473).powi(2),
    );
    check(
        &t,
        PE,
        "2 / [district=2]",
        "Exp(B)",
        2.218031641361f64.exp(),
    );
    check(
        &t,
        PE,
        "1 / km",
        "95% Confidence Interval for Exp(B) / Lower Bound",
        (-0.2403875092570f64 - 1.959963984540054 * 0.0782506635171).exp(),
    );
    check(&t, PE, "1 / [district=3]", "B", 0.0);
    check(&t, PE, "1 / [district=3]", "df", 0.0);
    assert_eq!(cell(&t, PE, "1 / Intercept", "Exp(B)"), &Cell::Empty);
    // rows observed, columns predicted
    check(&t, "Classification", "1", "Predicted / 1", 18.0);
    check(&t, "Classification", "1", "Predicted / 3", 9.0);
    check(&t, "Classification", "3", "Predicted / 2", 7.0);
    check(
        &t,
        "Classification",
        "Overall Percentage",
        "Predicted / Percent Correct",
        100.0 * 46.0 / 81.0,
    );
    // saved: case 1 (district 1, 5.7 km, bus) and case 18, whose mode is missing
    let close = |a: Option<f64>, e: f64| (a.unwrap() - e).abs() < 1e-7;
    let est1 = values(&engine, "EST1_1");
    let est3 = values(&engine, "EST3_1");
    assert!(close(est1[0], 0.562459903238) && close(est3[0], 0.351090420239));
    assert!(close(est1[17], 0.208794128557) && close(est3[17], 0.668280334567));
    assert_eq!(est1[5], None, "km is missing");
    assert_eq!(est1[30], None, "district is user-missing");
    assert_eq!(values(&engine, "PRE_1")[0], Some(1.0));
    assert_eq!(values(&engine, "PRE_1")[17], Some(3.0));
    assert!(close(values(&engine, "PCP_1")[0], 0.562459903238));
    let hit = values(&engine, "hit");
    assert!(close(hit[0], 0.351090420239));
    assert_eq!(hit[17], None, "no observed category");
}

#[test]
fn nomreg_kernel_drops_the_multinomial_constant() {
    let mut engine = Engine::default();
    let t = run(
        &mut engine,
        &format!("{DATA}NOMREG mode BY district WITH km /PRINT=MFI LRT KERNEL."),
    );
    check(&t, MFI, "Intercept Only", M2LL, 171.393986916);
    check(&t, MFI, "Final", M2LL, 146.526092102);
    check(&t, LRT, "km", REDUCED, 158.253739974);
}

#[test]
fn nomreg_weighted_with_a_covariate_by_factor_interaction_and_the_first_category_as_base() {
    let mut engine = Engine::default();
    let t = run(
        &mut engine,
        &format!(
            "{DATA}WEIGHT BY w.\nNOMREG mode (BASE=FIRST) BY district shift WITH km\n\
             /MODEL=km district shift km*shift /PRINT=CPS MFI PARAMETER LRT FIT SUMMARY CLASSTABLE."
        ),
    );
    check(&t, "Case Processing Summary", "Valid", "N", 90.0);
    check(&t, "Case Processing Summary", "Subpopulation", "N", 77.0);
    check(&t, MFI, "Intercept Only", M2LL, 187.645333117);
    check(&t, MFI, "Final", M2LL, 142.946632062);
    check(&t, MFI, "Final", CHI, 44.6987010543);
    check(&t, MFI, "Final", DF, 10.0);
    check(
        &t,
        "Goodness-of-Fit",
        "Pearson",
        "Chi-Square",
        153.191076055,
    );
    check(
        &t,
        "Goodness-of-Fit",
        "Deviance",
        "Chi-Square",
        141.755784579,
    );
    check(&t, "Pseudo R-Square", "Nagelkerke", "Value", 0.446263364962);
    check(&t, "Pseudo R-Square", "McFadden", "Value", 0.236872256126);
    check(&t, PE, "2 / Intercept", "B", -3.0002586282243);
    check(&t, PE, "2 / km", "B", 0.262681551899);
    check(&t, PE, "2 / [district=2]", "B", 2.960201264689);
    check(&t, PE, "2 / [shift=day]", "B", -1.14472658868);
    check(&t, PE, "2 / km * [shift=day]", "B", -0.0994815337891);
    check(&t, PE, "3 / km * [shift=day]", "B", 0.2090982973676);
    check(&t, PE, "3 / [district=1]", "Std. Error", 0.667626665925);
    check(&t, PE, "3 / km * [shift=day]", "Std. Error", 0.152043258026);
    check(&t, PE, "2 / km * [shift=night]", "df", 0.0);
    // without km the interaction spans the same space: no test
    check(&t, LRT, "km", CHI, 0.0);
    check(&t, LRT, "km", DF, 0.0);
    check(&t, LRT, "district", CHI, 23.5960933374);
    check(&t, LRT, "shift", CHI, 2.55922208495);
    check(&t, LRT, "km * shift", CHI, 3.5862670948);
    check(&t, LRT, "km * shift", DF, 2.0);
    check(&t, "Classification", "1", "Predicted / 3", 11.5);
    check(&t, "Classification", "2", "Predicted / 2", 8.5);
    check(&t, "Classification", "3", "Predicted / 3", 31.5);
}

#[test]
fn nomreg_without_intercept_compares_with_equal_probabilities() {
    let mut engine = Engine::default();
    let t = run(
        &mut engine,
        &format!(
            "{DATA}NOMREG mode BY district WITH km /INTERCEPT=EXCLUDE /PRINT=MFI PARAMETER SUMMARY."
        ),
    );
    check(&t, MFI, "Null", M2LL, 173.816307681);
    check(&t, MFI, "Final", M2LL, 142.367209019);
    check(&t, MFI, "Final", CHI, 31.449098662);
    check(&t, MFI, "Final", DF, 8.0);
    check(
        &t,
        "Pseudo R-Square",
        "Cox and Snell",
        "Value",
        0.321764344744,
    );
    check(&t, PE, "1 / [district=3]", "B", 1.22824423755);
    check(&t, PE, "2 / [district=1]", "B", -1.15701184312);
    check(&t, PE, "2 / [district=1]", "Std. Error", 0.986695105075);
}

#[test]
fn nomreg_string_dependent_in_descending_order_with_a_base_value() {
    // walk, bus, bike in descending order; bus as base gives the numbers of the first test
    let mut engine = Engine::default();
    let t = run(
        &mut engine,
        &format!(
            "{DATA}SELECT IF NOT MISSING(mode).\nSTRING how (A4).\n\
             RECODE mode (1='walk') (2='bike') (3='bus') INTO how.\n\
             NOMREG how (ORDER=DESCENDING BASE='bus') BY district WITH km /PRINT=PARAMETER CLASSTABLE /SAVE=PREDCAT."
        ),
    );
    check(&t, PE, "walk / km", "B", -0.2403875092570);
    check(&t, PE, "bike / [district=2]", "B", 2.218031641361);
    let table = t.iter().find(|t| t.title == "Classification").unwrap();
    assert_eq!(table.columns[1], "walk");
    assert_eq!(table.columns[3], "bike");
    let data = engine.dataset().unwrap();
    match data.variable("PRE_1").unwrap().values() {
        Values::String(v) => assert_eq!(v.get(0).trim_end(), "walk"),
        Values::Numeric(_) => panic!("a string category saves as a string"),
    }
}

#[test]
fn nomreg_explains_what_it_cannot_fit() {
    assert!(
        run_error(&format!("{DATA}NOMREG mode (BASE=7) BY district.")).contains("BASE value 7")
    );
    assert!(
        run_error(&format!("{DATA}NOMREG mode BY district WITH shift."))
            .contains("string variable")
    );
    assert!(
        run_error(&format!(
            "{DATA}SELECT IF mode = 1.\nNOMREG mode BY district."
        ))
        .contains("at least two categories")
    );
    assert!(
        run_error(&format!(
            "{DATA}NOMREG mode BY district WITH km /SUBPOP=district."
        ))
        .contains("SUBPOP")
    );
}
