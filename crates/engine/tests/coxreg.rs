//! COXREG on synthetic data: an invented follow-up of 64 subscription customers, months until
//! they cancel (`ended` = 1; 0 still active and 2 moved away are censored), with the plan (three
//! categories), the monthly spend, the site (two strata) and a case weight. Many cancellations
//! share a month, so the Breslow treatment of ties matters. The expected numbers come from R's
//! `survival` package, run as a black box: `coxph` with Breslow ties on the same coded columns,
//! `survfit` with the direct (Kalbfleisch-Prentice) estimate for the baseline survival, and the
//! `dfbeta` and `schoenfeld` residuals. Where the definitions differ the test converts, as the
//! comments say.

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

fn tables(source: &str) -> Vec<Table> {
    run(&mut Engine::default(), &format!("{DATA}{source}"))
}

fn run_error(source: &str) -> String {
    let (commands, errors) = parse(&format!("{DATA}{source}"));
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
            carried
                .iter()
                .map(String::as_str)
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" / ")
        })
        .collect()
}

/// The number in the last table with this title at the row and column.
fn value(tables: &[Table], title: &str, row: &str, column: &str) -> f64 {
    for table in tables.iter().rev().filter(|t| t.title == title) {
        let Some(r) = row_keys(table).iter().position(|k| k == row) else {
            continue;
        };
        let c = table
            .column_titles()
            .iter()
            .position(|t| t == column)
            .unwrap_or_else(|| {
                panic!("{title}: no column {column} in {:?}", table.column_titles())
            });
        return match &table.rows[r][c] {
            Cell::Number(n) => *n,
            other => panic!("{title} {row} {column}: not a number: {other:?}"),
        };
    }
    panic!("no table {title} with row {row}");
}

/// The row of a table without row headers whose first cell is `first`.
fn row_of<'t>(table: &'t Table, first: f64) -> &'t [Cell] {
    table
        .rows
        .iter()
        .find(|r| matches!(r[0], Cell::Number(n) if n == first))
        .unwrap_or_else(|| panic!("{}: no row {first}", table.title))
}

fn num(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        other => panic!("not a number: {other:?}"),
    }
}

#[track_caller]
fn near(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance * expected.abs().max(1.0),
        "{actual} is not {expected}"
    );
}

fn values(engine: &Engine, name: &str) -> Vec<Option<f64>> {
    let data = engine.dataset().unwrap();
    match data.variable(name).unwrap().values() {
        Values::Numeric(v) => (0..data.case_count()).map(|k| v.get(k)).collect(),
        Values::String(_) => panic!("{name} is a string"),
    }
}

/// months, ended, plan, spend, site, w; the last three cases are dropped: a missing spend, a
/// negative time and a censored case before the first event.
const DATA: &str = "DATA LIST FREE /months ended plan spend site w.
BEGIN DATA.
4 1 1 51.7 1 1   4 1 1 54.6 1 1   19 0 2 34.5 1 1   9 1 1 44.1 1 1
8 1 1 30.7 1 2   4 1 1 51.1 1 1   14 2 3 53 1 0.5   10 1 2 21.9 1 1
5 1 2 56.4 1 0.5   8 1 1 34.5 1 1   4 1 3 41.8 1 1   11 2 2 57.7 1 0.5
27 2 2 35.2 1 1   15 1 1 56.4 1 1   27 0 3 38.2 1 1   9 0 2 28.2 1 2
16 1 2 56.6 1 1   12 1 2 48.1 1 1   28 2 3 29.5 1 1   9 1 3 37 1 1
5 0 1 28.1 1 2   15 1 1 38.1 1 1   2 1 3 34.2 1 1   21 1 1 29.7 1 1
5 2 2 35 1 1   19 0 3 42.1 1 1   5 1 1 29.7 1 0.5   17 2 2 39.1 1 0.5
9 1 1 60.7 1 2   2 1 2 21.4 1 2   5 0 1 37.4 1 0.5   19 2 3 43.6 1 1
18 0 3 52.8 2 2   11 1 1 38 2 1   5 0 3 21.2 2 1   12 1 1 43.4 2 1
10 2 2 41.2 2 2   6 2 3 39.2 2 2   3 1 1 61.1 2 1   22 2 3 22.6 2 0.5
9 1 3 34.5 2 1   5 1 1 19 2 1   9 0 3 43.3 2 1   15 1 3 34.8 2 0.5
14 0 1 26.8 2 1   19 1 3 42.2 2 1   19 0 2 50.1 2 1   5 1 2 55.9 2 1
12 2 3 35.7 2 1   20 1 2 44.4 2 1   4 1 2 24.4 2 1   8 0 3 30.8 2 1
10 1 3 49.3 2 1   4 1 1 40 2 1   3 1 2 17.7 2 0.5   8 1 1 41.7 2 1
19 0 3 40 2 2   17 2 3 17.4 2 1   24 1 2 35.5 2 0.5   4 1 1 35.9 2 1
5 1 1 44 2 1   11 1 1 53.4 2 1   2 1 1 37.7 2 1   1 1 3 42.1 2 0.5
5 1 2 . 1 1   -2 1 1 40 2 1   0.5 0 3 40 1 1
END DATA.
";

/// Iterations to full convergence, as R runs them; the default criteria stop a little earlier.
const TIGHT: &str = "/CRITERIA=BCON(1E-12) LCON(0) ITERATE(50).";

#[test]
fn enter_with_deviation_coding_matches_r() {
    let t = tables(
        "COXREG months WITH plan spend /STATUS=ended(1) /CATEGORICAL=plan /PRINT=CI(95) BASELINE
          /CRITERIA=BCON(1E-12) LCON(0).",
    );
    let cps = "Case Processing Summary";
    near(
        value(&t, cps, "Cases available in analysis / Event", "N"),
        39.0,
        0.0,
    );
    near(
        value(&t, cps, "Cases available in analysis / Censored", "N"),
        25.0,
        0.0,
    );
    near(
        value(&t, cps, "Cases dropped / Cases with missing values", "N"),
        1.0,
        0.0,
    );
    near(
        value(&t, cps, "Cases dropped / Cases with negative time", "N"),
        1.0,
        0.0,
    );
    let early = "Cases dropped / Censored cases before the earliest event in a stratum";
    near(value(&t, cps, early, "N"), 1.0, 0.0);
    // deviation coding: the last plan is -1 on both columns
    let codings = "Categorical Variable Codings";
    near(
        value(&t, codings, "plan / 3", "Parameter coding / (1)"),
        -1.0,
        1e-12,
    );
    near(value(&t, codings, "plan / 3", "Frequency"), 22.0, 0.0);
    let block0 = t
        .iter()
        .find(|t| t.title == "Omnibus Tests of Model Coefficients")
        .unwrap();
    near(num(&block0.rows[0][0]), 275.8461041115, 1e-9);
    let omnibus = "Omnibus Tests of Model Coefficients";
    near(
        value(&t, omnibus, "Step 1", "-2 Log Likelihood"),
        261.3617866911,
        1e-9,
    );
    near(
        value(&t, omnibus, "Step 1", "Overall (score) / Chi-square"),
        15.64687006851,
        1e-7,
    );
    let eq = "Variables in the Equation";
    near(
        value(&t, eq, "Step 1 / plan(1)", "B"),
        0.80632561355487,
        1e-6,
    );
    near(
        value(&t, eq, "Step 1 / plan(2)", "B"),
        -0.19153675560431,
        1e-6,
    );
    near(value(&t, eq, "Step 1 / spend", "B"), 0.01195809997402, 1e-6);
    near(
        value(&t, eq, "Step 1 / plan(1)", "SE"),
        0.22676763084064,
        1e-6,
    );
    near(
        value(&t, eq, "Step 1 / spend", "SE"),
        0.01583652715434,
        1e-6,
    );
    near(value(&t, eq, "Step 1 / plan", "Wald"), 13.00684550483, 1e-6);
    near(value(&t, eq, "Step 1 / plan", "df"), 2.0, 0.0);
    near(
        value(&t, eq, "Step 1 / spend", "95% CI for Exp(B) / Lower"),
        0.981099965587,
        1e-6,
    );
    near(
        value(&t, eq, "Step 1 / spend", "95% CI for Exp(B) / Upper"),
        1.043934891275,
        1e-6,
    );
    near(
        value(&t, "Covariate Means", "spend", "Mean"),
        39.31875,
        1e-12,
    );
    // R's survfit: stype=1 survival; its SE as survival times the std.err of stype=2 (the
    // Breslow cumulative hazard); the cumulative hazard as -log of that survival; the baseline at
    // covariates 0 as that hazard times exp(-mean'b)
    let table = t.iter().find(|t| t.title == "Survival Table").unwrap();
    for (time, surv, se, cum, base) in [
        (
            1.0,
            0.9874005787447,
            0.01255390643498,
            0.01267946702765,
            0.007634196160295,
        ),
        (
            4.0,
            0.8300671419473,
            0.04336873648636,
            0.18624868755082,
            0.112138705219979,
        ),
        (
            24.0,
            0.1327410328197,
            0.07082910160382,
            2.01935517052236,
            1.215836080132693,
        ),
    ] {
        let row = row_of(table, time);
        near(num(&row[1]), base, 1e-6);
        near(num(&row[2]), surv, 1e-6);
        near(num(&row[3]), se, 1e-6);
        near(num(&row[4]), cum, 1e-6);
    }
}

#[test]
fn strata_and_indicator_coding_match_r() {
    let t = tables(&format!(
        "COXREG months WITH plan spend /STATUS=ended(1) /STRATA=site
          /CATEGORICAL=plan /CONTRAST(plan)=INDICATOR(1) /PRINT=BASELINE {TIGHT}"
    ));
    let eq = "Variables in the Equation";
    near(
        value(&t, eq, "Step 1 / plan(1)", "B"),
        -1.09342062977244,
        1e-6,
    );
    near(
        value(&t, eq, "Step 1 / plan(2)", "B"),
        -1.39369105486327,
        1e-6,
    );
    near(value(&t, eq, "Step 1 / spend", "B"), 0.01198165733053, 1e-6);
    near(
        value(&t, eq, "Step 1 / plan(2)", "SE"),
        0.43813900487990,
        1e-6,
    );
    let omnibus = "Omnibus Tests of Model Coefficients";
    near(
        value(&t, omnibus, "Step 1", "-2 Log Likelihood"),
        206.7122407939,
        1e-9,
    );
    near(
        value(&t, omnibus, "Step 1", "Overall (score) / Chi-square"),
        15.48495506693,
        1e-7,
    );
    let second: Vec<&Table> = t.iter().filter(|t| t.title == "Survival Table").collect();
    assert_eq!(second.len(), 2);
    assert_eq!(second[1].caption.as_deref(), Some("site = 2"));
    for (time, surv, se, cum, base) in [
        (
            1.0,
            0.9732315578193,
            0.02634193351450,
            0.02713324173944,
            0.03719824830913,
        ),
        (
            3.0,
            0.8899989623795,
            0.05104883560663,
            0.11653498212236,
            0.15976333544354,
        ),
    ] {
        let row = row_of(second[1], time);
        near(num(&row[1]), base, 1e-6);
        near(num(&row[2]), surv, 1e-6);
        near(num(&row[3]), se, 1e-6);
        near(num(&row[4]), cum, 1e-6);
    }
}

#[test]
fn case_weights_match_r() {
    let t = tables(&format!(
        "WEIGHT BY w.\nCOXREG months WITH plan spend /STATUS=ended(1) /CATEGORICAL=plan {TIGHT}"
    ));
    let eq = "Variables in the Equation";
    near(
        value(&t, eq, "Step 1 / plan(1)", "B"),
        0.859658454310029,
        1e-6,
    );
    near(
        value(&t, eq, "Step 1 / spend", "B"),
        0.007642202377226,
        1e-6,
    );
    // R gives a robust variance with fractional weights; this is its naive.var
    near(
        value(&t, eq, "Step 1 / plan(1)", "SE"),
        0.22813403591715,
        1e-6,
    );
    let omnibus = "Omnibus Tests of Model Coefficients";
    near(
        value(&t, omnibus, "Step 1", "-2 Log Likelihood"),
        263.4758497895,
        1e-9,
    );
    near(
        value(&t, omnibus, "Step 1", "Overall (score) / Chi-square"),
        17.90407146196,
        1e-7,
    );
}

#[test]
fn saved_case_statistics_match_r() {
    let mut engine = Engine::default();
    run(
        &mut engine,
        &format!(
            "{DATA}COXREG months WITH plan spend /STATUS=ended(1) /STRATA=site /CATEGORICAL=plan
              /SAVE=SURVIVAL SE HAZARD LML XBETA DFBETA PRESID {TIGHT}"
        ),
    );
    let sur = values(&engine, "SUR_1");
    let se = values(&engine, "SE_1");
    let haz = values(&engine, "HAZ_1");
    let lml = values(&engine, "LML_1");
    let xbe = values(&engine, "XBE_1");
    let dfb: Vec<Vec<Option<f64>>> = (1..=3)
        .map(|j| values(&engine, &format!("DFB{j}_1")))
        .collect();
    let pre: Vec<Vec<Option<f64>>> = (1..=3)
        .map(|j| values(&engine, &format!("PRE{j}_1")))
        .collect();
    // survival and its SE as in the survival table; the hazard as -log of the survival; R's
    // dfbeta is the same one-step change b minus b without the case
    for (k, s, e, h, x, d) in [
        (
            0,
            0.6719788249421,
            0.1090770319731,
            0.3975284494582,
            0.9349537470563,
            [0.013415366066110, -0.012918713287043, 0.001335738627765],
        ),
        (
            2,
            0.5072951132551,
            0.1308788518884,
            0.6786623673061,
            -0.3645513888012,
            [0.025406704831356, -0.048455559340689, 0.001411432530885],
        ),
        (
            39,
            0.4226815063249,
            0.1671619470977,
            0.8611363236679,
            -0.8074035361253,
            [0.014816064687226, 0.039606532418409, 0.002551811212496],
        ),
    ] {
        near(sur[k].unwrap(), s, 1e-6);
        near(se[k].unwrap(), e, 1e-6);
        near(haz[k].unwrap(), h, 1e-6);
        near(lml[k].unwrap(), h.ln(), 1e-6);
        near(xbe[k].unwrap(), x, 1e-6);
        for j in 0..3 {
            near(dfb[j][k].unwrap(), d[j], 1e-5);
        }
    }
    // Schoenfeld residuals of an event (R names its rows by time; ties share the risk-set
    // mean, which gives case 1's row); none for a censored case or a dropped one
    for (j, r) in [0.37305249926835, -0.09010014693505, 8.45163331843301]
        .into_iter()
        .enumerate()
    {
        near(pre[j][0].unwrap(), r, 1e-6);
        assert_eq!(pre[j][2], None);
    }
    assert_eq!(sur[64], None);
}
