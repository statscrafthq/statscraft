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
fn row_of(table: &Table, first: f64) -> &[Cell] {
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

#[test]
fn forward_stepwise_scores_match_r() {
    let t = tables(&format!(
        "COXREG months WITH plan spend site /STATUS=ended(1) /CATEGORICAL=plan /METHOD=FSTEP(LR) {TIGHT}"
    ));
    let not_in = "Variables not in the Equation";
    // block 0: score tests at b = 0
    let block0 = t.iter().find(|t| t.title == not_in).unwrap();
    let keys = row_keys(block0);
    let at = |key: &str| num(&block0.rows[keys.iter().position(|k| k == key).unwrap()][2]);
    near(at("Step 0 / plan"), 15.08410446512, 1e-7);
    near(at("Step 0 / spend"), 1.115641647733, 1e-7);
    near(at("Step 0 / site"), 0.2327510716202, 1e-7);
    // plan enters; spend and site stay out
    near(
        value(&t, not_in, "Step 1 / spend", "Score"),
        0.5714025873041,
        1e-6,
    );
    near(
        value(&t, not_in, "Step 1 / site", "Score"),
        1.106055238518,
        1e-6,
    );
    near(
        value(&t, not_in, "Step 1 / Residual Chi Square", "Score"),
        1.817669835563,
        1e-6,
    );
    let omnibus = "Omnibus Tests of Model Coefficients";
    near(
        value(&t, omnibus, "Step 1", "-2 Log Likelihood"),
        261.931985375,
        1e-9,
    );
    let eq = "Variables in the Equation";
    near(
        value(&t, eq, "Step 1 / plan(1)", "B"),
        0.8112524943775,
        1e-6,
    );
    assert!(
        t.iter()
            .filter(|t| t.title == eq)
            .all(|t| !row_keys(t).iter().any(|k| k.contains("spend")))
    );
}

#[test]
fn backward_removal_statistics_match_r() {
    for (method, plan, spend, site) in [
        ("COND", 14.23127448898, 0.7172620287832, 1.241851099585),
        ("WALD", 13.73579905479, 0.7147031828356, 1.243976700992),
        ("LR", 14.21798813504, 0.7172160223896, 1.241783130596),
    ] {
        let t = tables(&format!(
            "COXREG months WITH plan spend site /STATUS=ended(1) /CATEGORICAL=plan
              /METHOD=BSTEP({method}) {TIGHT}"
        ));
        let omnibus = "Omnibus Tests of Model Coefficients";
        let first = t.iter().filter(|t| t.title == omnibus).nth(1).unwrap();
        near(num(&first.rows[0][1]), 260.1200035605, 1e-9);
        if method == "WALD" {
            // the Wald statistics are the rows of Variables in the Equation
            let eq = "Variables in the Equation";
            let table = t.iter().find(|t| t.title == eq).unwrap();
            let keys = row_keys(table);
            let wald = |key: &str| num(&table.rows[keys.iter().position(|k| k == key).unwrap()][4]);
            near(wald("Step 1 / plan"), plan, 1e-6);
            near(wald("Step 1 / spend"), spend, 1e-6);
            near(wald("Step 1 / site"), site, 1e-6);
        } else {
            let removed = "Model if Term Removed";
            let table = t.iter().find(|t| t.title == removed).unwrap();
            let keys = row_keys(table);
            let loss = |key: &str| num(&table.rows[keys.iter().position(|k| k == key).unwrap()][2]);
            near(loss("Step 1 / plan"), plan, 1e-6);
            near(loss("Step 1 / spend"), spend, 1e-6);
            near(loss("Step 1 / site"), site, 1e-6);
        }
        // spend and then site leave; plan stays
        near(
            value(&t, omnibus, "Step 3", "-2 Log Likelihood"),
            261.931985375,
            1e-9,
        );
    }
}

#[test]
fn second_block_changes_from_step_and_block() {
    let t = tables(&format!(
        "COXREG months WITH plan spend /STATUS=ended(1) /CATEGORICAL=plan
          /METHOD=ENTER plan /METHOD=ENTER spend {TIGHT}"
    ));
    let omnibus: Vec<&Table> = t
        .iter()
        .filter(|t| t.title == "Omnibus Tests of Model Coefficients")
        .collect();
    let block2 = omnibus[2];
    let column = |name: &str| {
        let c = block2
            .column_titles()
            .iter()
            .position(|t| t == name)
            .unwrap();
        num(&block2.rows[0][c])
    };
    near(column("-2 Log Likelihood"), 261.3617866911, 1e-9);
    // the overall score test of plan and spend at b = 0
    near(column("Overall (score) / Chi-square"), 15.64687006851, 1e-7);
    near(column("Overall (score) / df"), 3.0, 0.0);
    near(
        column("Change From Previous Step / Chi-square"),
        261.931985375 - 261.3617866911,
        1e-7,
    );
    near(column("Change From Previous Block / df"), 1.0, 0.0);
}

#[test]
fn time_dependent_covariate_matches_r() {
    let mut engine = Engine::default();
    let t = run(
        &mut engine,
        &format!(
            "{DATA}TIME PROGRAM.
COMPUTE spend_t = spend * LN(T_ + 1).
COXREG months WITH plan spend spend_t /STATUS=ended(1) /CATEGORICAL=plan /SAVE=DFBETA {TIGHT}"
        ),
    );
    // R: coxph with tt(spend) = spend * log(t + 1), and dfbeta of the counting-process form
    // collapsed by case
    let eq = "Variables in the Equation";
    near(
        value(&t, eq, "Step 1 / plan(1)", "B"),
        0.82229843031116,
        1e-6,
    );
    near(
        value(&t, eq, "Step 1 / spend", "B"),
        -0.08181003183942,
        1e-6,
    );
    near(
        value(&t, eq, "Step 1 / spend_t", "B"),
        0.04797026618358,
        1e-6,
    );
    near(
        value(&t, eq, "Step 1 / spend_t", "SE"),
        0.02818261751005,
        1e-6,
    );
    let omnibus = "Omnibus Tests of Model Coefficients";
    near(
        value(&t, omnibus, "Step 1", "-2 Log Likelihood"),
        258.3866149926,
        1e-9,
    );
    near(
        value(&t, omnibus, "Step 1", "Overall (score) / Chi-square"),
        18.67129144839,
        1e-7,
    );
    for (k, d) in [
        (
            0,
            [
                0.014993823477150,
                -0.009491832994857,
                0.004798384021318,
                -0.001449017980813,
            ],
        ),
        (
            2,
            [
                0.021571324379048,
                -0.047155556512628,
                -0.002726249658188,
                0.002118283694588,
            ],
        ),
    ] {
        for (j, expected) in d.into_iter().enumerate() {
            near(
                values(&engine, &format!("DFB{}_1", j + 1))[k].unwrap(),
                expected,
                1e-5,
            );
        }
    }
    // the codings count each case once, not once per event time
    let codings = "Categorical Variable Codings";
    near(value(&t, codings, "plan / 3", "Frequency"), 22.0, 0.0);
    // the time program ran with COXREG and is gone; its variables never reach the data
    assert!(engine.dataset().unwrap().variable("spend_t").is_err());
    assert!(run_error("COXREG months WITH spend_t /STATUS=ended(1).").contains("spend_t"));
}

#[test]
fn plots_follow_the_patterns() {
    use statscraft_engine::plot::Plot;
    let mut engine = Engine::default();
    let t = run(
        &mut engine,
        &format!(
            "{DATA}COXREG months WITH plan spend /STATUS=ended(1) /CATEGORICAL=plan
              /PLOT=SURVIVAL LML /PATTERN=spend(50) BY plan {TIGHT}"
        ),
    );
    let charts = engine.take_charts();
    let titles: Vec<&str> = charts.iter().map(|c| c.title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "Survival Function at mean of covariates",
            "Survival Function for pattern 1",
            "LML Function at mean of covariates",
            "LML Function for pattern 1",
        ]
    );
    let groups = |c: usize| match &charts[c].plot {
        Some(Plot::Points { groups, .. }) => groups.clone(),
        other => panic!("{other:?}"),
    };
    // a step: the level before and after each event time
    let mean = groups(0);
    let at4: Vec<f64> = mean[0]
        .points
        .iter()
        .filter(|p| p.x == 4.0)
        .map(|p| p.y)
        .collect();
    near(at4[1], 0.8300671419473, 1e-6);
    assert_eq!(mean[0].points[0].x, 0.0);
    // R: survfit at plan 1 (coded 1, 0) and plan 3 (-1, -1) with spend 50
    let pattern = groups(1);
    assert_eq!(pattern.len(), 3);
    assert_eq!(pattern[0].name, "plan = 1");
    for (g, expected) in [(0, 0.6333875517073), (2, 0.8956001158324)] {
        let y = pattern[g].points.iter().rfind(|p| p.x == 4.0).unwrap().y;
        near(y, expected, 1e-6);
    }
    near(
        value(&t, "Covariate Means", "spend", "Pattern 1"),
        50.0,
        0.0,
    );
}

#[test]
fn errors_and_unsupported_parts() {
    assert!(run_error("COXREG months WITH spend /STATUS=plan('a').").contains("numbers"));
    assert!(run_error("COXREG months WITH months /STATUS=ended(1).").contains("survival time"));
    assert!(run_error("COXREG months WITH spend /STATUS=ended(9).").contains("at least one event"));
    assert!(
        run_error("COXREG months WITH spend /STATUS=ended(1) /PATTERN=plan(1).")
            .contains("not a covariate")
    );
    assert!(
        run_error(
            "TIME PROGRAM.\nCOMPUTE z = spend * T_.\nCOXREG months WITH z T_ /STATUS=ended(1)."
        )
        .contains("T_")
    );
}

#[test]
fn null_model_gives_the_initial_likelihood_and_the_baseline() {
    let t = tables("COXREG months /STATUS=ended(1) /PRINT=BASELINE.");
    let block0 = t
        .iter()
        .find(|t| t.title == "Omnibus Tests of Model Coefficients")
        .unwrap();
    // the case missing only on spend is in: R's coxph with no covariates on 65 cases
    near(num(&block0.rows[0][0]), 284.373710197, 1e-9);
    // without covariates the baseline survival is the Kaplan-Meier estimate (R's survfit)
    let table = t.iter().find(|t| t.title == "Survival Table").unwrap();
    near(num(&row_of(table, 5.0)[2]), 0.7076923076923, 1e-12);
}

#[test]
fn a_time_program_may_change_a_covariate_of_the_data() {
    // R: the counting-process form with spend + 10 after month 12
    let mut engine = Engine::default();
    let t = run(
        &mut engine,
        &format!(
            "{DATA}SELECT IF NOT MISSING(spend).
TIME PROGRAM.
IF (T_ > 12) spend = spend + 10.
COXREG months WITH spend /STATUS=ended(1) {TIGHT}"
        ),
    );
    let eq = "Variables in the Equation";
    near(value(&t, eq, "Step 1 / spend", "B"), 0.01687282043637, 1e-6);
    // the data keeps its own values
    assert_eq!(values(&engine, "spend")[0], Some(51.7));
    // a missing value from the program where the case is otherwise valid is an error
    assert!(
        run_error(
            "TIME PROGRAM.\nCOMPUTE spend = spend + T_.\nCOXREG months WITH spend /STATUS=ended(1)."
        )
        .contains("missing")
    );
}

#[test]
fn a_covariate_constant_within_strata_is_left_out() {
    let mut engine = Engine::default();
    let t = run(
        &mut engine,
        &format!(
            "{DATA}COMPUTE size = site * 10.\nCOXREG months WITH spend size /STATUS=ended(1) /STRATA=site."
        ),
    );
    assert!(
        engine
            .warnings()
            .iter()
            .any(|w| w.contains("size is a linear combination"))
    );
    let eq = t
        .iter()
        .find(|t| t.title == "Variables in the Equation")
        .unwrap();
    assert!(row_keys(eq).iter().all(|k| !k.contains("size")));
}

#[test]
fn backward_stepping_may_remove_every_term() {
    // site alone has a score p of 0.63, above POUT, so BSTEP takes it out again
    let t = tables(&format!(
        "COXREG months WITH site /STATUS=ended(1) /METHOD=BSTEP(LR) {TIGHT}"
    ));
    let omnibus = "Omnibus Tests of Model Coefficients";
    // back at the null model of the 65 cases valid on site
    near(
        value(&t, omnibus, "Step 2", "-2 Log Likelihood"),
        284.373710197,
        1e-9,
    );
}

#[test]
fn time_program_edge_cases() {
    // CLEAR TRANSFORMATIONS drops a waiting time program too
    assert!(
        run_error(
            "TIME PROGRAM.\nCOMPUTE spend_t = spend * T_.\nCLEAR TRANSFORMATIONS.\n\
             COXREG months WITH spend_t /STATUS=ended(1)."
        )
        .contains("spend_t")
    );
    // a program that never changes the covariate is not used, and the case missing on spend is
    // dropped as usual
    let mut engine = Engine::default();
    let t = run(
        &mut engine,
        &format!(
            "{DATA}TIME PROGRAM.\nIF (T_ > 1000) spend = 1.\n\
             COXREG months WITH plan spend /STATUS=ended(1) /CATEGORICAL=plan {TIGHT}"
        ),
    );
    assert!(engine.warnings().iter().any(|w| w.contains("changes none")));
    near(
        value(&t, "Variables in the Equation", "Step 1 / spend", "B"),
        0.01195809997402,
        1e-6,
    );
}

#[test]
fn string_categories_in_a_pattern_and_dfbeta_out_of_the_model() {
    let mut engine = Engine::default();
    run(
        &mut engine,
        &format!(
            "{DATA}STRING tier (A6).
RECODE plan (1='basic') (2='plus') (ELSE='top') INTO tier.
COXREG months WITH tier spend site /STATUS=ended(1) /METHOD=FSTEP(LR)
  /PLOT=SURVIVAL /PATTERN=tier('plus') /SAVE=DFBETA {TIGHT}"
        ),
    );
    // tier is the only term in the final model: spend and site have empty DFBETA columns
    assert!(values(&engine, "DFB1_1")[0].is_some());
    assert!(values(&engine, "DFB3_1").iter().all(Option::is_none));
    assert_eq!(engine.take_charts().len(), 2);
}

#[test]
fn covariate_lists_take_to() {
    // plan TO site spans plan, spend and site in the data
    let t = tables(&format!(
        "COXREG months WITH plan TO site /STATUS=ended(9) /STATUS=ended(1)
          /CATEGORICAL=plan /METHOD=BSTEP(WALD) {TIGHT}"
    ));
    let omnibus = "Omnibus Tests of Model Coefficients";
    near(
        value(&t, omnibus, "Step 1", "-2 Log Likelihood"),
        260.1200035605,
        1e-9,
    );
}
