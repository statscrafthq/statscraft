//! CSCOXREG on an invented survey of 84 patients: 4 strata (str), 3 clinics drawn in each
//! (psu, from npsu clinics), a weight, start and end times (t0, t) with ties, a status (1 is the
//! event; 0 and 2 are censored), a three-level factor grp, a covariate x, a baseline stratum bs
//! and a domain flag dom. One record has no x, one a negative time, and one a start time equal to
//! its end time.
//!
//! The expected numbers come from R's survival and survey packages used as a black box:
//! coxph with the weights on the valid records (grp releveled so that its last level is the
//! reference), its unweighted score residuals put at 0 on every other design record, and
//! J^-1 V J^-1 with V the design covariance of the total of w U from svytotal on a svydesign of
//! all 84 records (ids=~psu, strata=~str, nest=TRUE, and fpc=~npsu for the plan without
//! replacement). svycoxph on the same design, subset to the analysed records, gives the same
//! estimates and standard errors to 1e-12. Design effects divide by J^-1 S J^-1 with
//! S = (1 - n/N) N/(n - 1) sum w U U' over the 84 records, and the Wald F tests use the manual's
//! (df - r + 1)/(r df) X2 on r and df - r + 1 with df = 12 clinics - 4 strata.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const RECORDS: &str = "
1,1,8,28.5,0,3,1,1,6.4,1,1 1,1,8,27,15,28,1,2,5.5,2,1 1,1,8,28.5,0,25,2,1,4.6,1,1 1,1,8,27,0,2,1,3,7.4,1,1
1,1,8,28.5,0,11,0,3,5.6,2,0 1,1,8,27,0,2,1,1,,2,1 1,1,8,27.5,0,7,1,2,7.0,2,1 1,2,8,29,0,13,1,2,4.8,1,0
1,2,8,30.5,3,21,1,1,5.9,1,1 1,2,8,29.5,0,14,1,2,4.3,1,1 1,2,8,29,0,11,2,2,3.1,1,0 1,2,8,29,0,9,1,2,8.1,2,1
1,2,8,29.5,0,11,1,3,2.4,2,1 1,2,8,29.5,4,10,0,3,5.8,1,0 1,3,8,31.5,11,13,1,2,6.9,2,1 1,3,8,31,0,32,2,2,3.2,2,1
1,3,8,32.5,0,8,1,1,7.5,1,1 1,3,8,31.5,0,-3,1,1,5.9,1,1 1,3,8,31.5,9,13,0,1,2.5,2,0 1,3,8,32.5,0,15,2,1,3.7,1,0
1,3,8,31,0,19,1,3,3.7,1,1 2,1,10,32.5,0,4,1,1,4.8,1,1 2,1,10,33.5,0,7,1,2,5.2,2,1 2,1,10,32.5,0,7,1,2,6.7,2,0
2,1,10,32.5,0,9,1,3,4.2,2,1 2,1,10,32,3,12,1,3,8.0,2,1 2,1,10,32,0,7,1,3,7.6,2,0 2,1,10,32.5,0,17,1,1,6.7,2,1
2,2,10,35.5,0,24,0,3,7.6,1,0 2,2,10,34,2,5,1,3,4.2,2,1 2,2,10,34.5,13,13,1,3,5.9,2,1 2,2,10,34.5,0,14,1,1,4.6,2,1
2,2,10,35.5,3,9,0,3,5.3,1,1 2,2,10,35.5,0,20,2,1,6.0,1,0 2,2,10,34.5,0,9,1,1,5.3,1,1 2,3,10,36.5,0,23,1,2,8.4,2,1
2,3,10,36.5,3,7,1,1,4.9,2,1 2,3,10,36,0,12,0,1,5.9,1,1 2,3,10,37.5,0,17,1,2,5.3,1,1 2,3,10,37.5,5,6,1,1,9.2,1,1
2,3,10,37.5,1,2,1,3,6.1,2,1 2,3,10,37.5,0,4,1,2,3.5,1,1 3,1,6,37.5,0,9,1,1,7.9,1,0 3,1,6,37,16,25,1,2,3.7,2,0
3,1,6,38.5,0,14,1,1,5.6,1,0 3,1,6,38.5,0,36,0,2,4.0,1,0 3,1,6,37.5,1,8,1,2,7.6,2,1 3,1,6,37,2,17,1,1,5.8,2,1
3,1,6,37.5,0,3,1,2,7.9,2,1 3,2,6,39,6,8,1,3,6.3,2,1 3,2,6,39.5,0,7,1,3,7.7,2,1 3,2,6,39,0,15,1,3,4.8,2,1
3,2,6,40.5,0,16,1,1,6.8,1,1 3,2,6,40.5,0,7,1,3,4.6,1,1 3,2,6,39,0,11,1,2,5.3,2,1 3,2,6,39,2,17,1,1,4.7,1,0
3,3,6,41.5,5,8,1,2,5.2,1,0 3,3,6,41,3,8,1,2,7.7,1,0 3,3,6,42.5,0,23,0,3,4.5,1,1 3,3,6,42.5,0,1,1,3,6.6,1,1
3,3,6,42.5,0,17,2,3,5.3,1,1 3,3,6,41,0,9,1,1,4.4,2,1 3,3,6,41,0,12,0,3,5.4,1,1 4,1,9,42.5,0,4,1,3,11.3,1,1
4,1,9,43.5,8,10,1,1,6.1,1,0 4,1,9,43.5,0,3,1,3,6.2,2,1 4,1,9,42.5,1,15,1,3,6.6,1,1 4,1,9,43.5,0,7,1,3,4.0,2,1
4,1,9,43.5,0,7,1,1,3.6,2,1 4,1,9,42,0,1,1,3,7.0,2,1 4,2,9,45.5,0,12,1,3,6.4,2,1 4,2,9,44.5,0,6,1,1,4.9,1,1
4,2,9,44.5,6,17,1,3,4.8,1,1 4,2,9,44.5,0,1,1,1,5.5,1,1 4,2,9,45.5,0,7,1,1,6.4,1,1 4,2,9,44,3,5,1,1,6.6,1,1
4,2,9,44.5,0,4,1,1,5.1,2,1 4,3,9,46.5,0,2,1,1,5.8,2,1 4,3,9,46,0,1,1,3,5.1,2,0 4,3,9,47.5,0,7,0,1,4.4,1,1
4,3,9,47.5,0,8,1,3,7.1,1,1 4,3,9,47.5,0,6,1,2,7.1,2,1 4,3,9,46.5,6,17,0,2,6.6,1,0 4,3,9,46,0,7,1,3,6.4,1,1
";

/// The data, the two plans (clinics with and without replacement) in a fresh folder, and then
/// `commands`.
fn program(folder: &str, commands: &str) -> String {
    let dir =
        std::env::temp_dir().join(format!("statscraft-cscox-{folder}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let records: Vec<&str> = RECORDS.split_whitespace().collect();
    format!(
        "DATA LIST LIST(\",\") /str psu npsu w t0 t st grp x bs dom.\n\
         BEGIN DATA\n{}\nEND DATA.\nCD '{}'.\n\
         CSPLAN ANALYSIS /PLAN FILE='wr.json' /PLANVARS ANALYSISWEIGHT=w\n\
         \x20 /DESIGN STRATA=str CLUSTER=psu /ESTIMATOR TYPE=WR.\n\
         CSPLAN ANALYSIS /PLAN FILE='wor.json' /PLANVARS ANALYSISWEIGHT=w\n\
         \x20 /DESIGN STRATA=str CLUSTER=psu /ESTIMATOR TYPE=EQUAL_WOR /POPSIZE VARIABLE=npsu.\n\
         {commands}\n",
        records.join("\n"),
        dir.display()
    )
}

fn run(source: &str) -> (Engine, Vec<Table>) {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        match engine.execute(&command.command) {
            Ok(more) => tables.extend(more),
            Err(error) => panic!("line {}: {error}", command.first_line),
        }
    }
    (engine, tables)
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

fn find<'t>(tables: &'t [Table], title: &str) -> &'t Table {
    tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"))
}

/// The first text cells of each row joined with " / ", an empty one taking the one above.
fn row_keys(table: &Table) -> Vec<String> {
    let mut carried = vec![String::new(); table.row_headers];
    table
        .rows
        .iter()
        .map(|row| {
            for (level, cell) in row.iter().take(table.row_headers).enumerate() {
                if let Cell::Text(t) = cell
                    && !t.is_empty()
                {
                    carried[level] = t.clone();
                    for deeper in carried.iter_mut().skip(level + 1) {
                        deeper.clear();
                    }
                }
            }
            carried
                .iter()
                .filter(|s| !s.is_empty())
                .cloned()
                .collect::<Vec<_>>()
                .join(" / ")
        })
        .collect()
}

fn value(table: &Table, row: &str, column: &str) -> f64 {
    let r = row_keys(table)
        .iter()
        .position(|k| k == row)
        .unwrap_or_else(|| panic!("{}: no row {row} in {:?}", table.title, row_keys(table)));
    let titles = table.column_titles();
    let c = titles
        .iter()
        .position(|t| t == column)
        .unwrap_or_else(|| panic!("{}: no column {column} in {titles:?}", table.title));
    match &table.rows[r][c] {
        Cell::Number(n) => *n,
        other => panic!(
            "{} / {row} / {column}: not a number: {other:?}",
            table.title
        ),
    }
}

/// Several cells of one table at a relative tolerance of 1e-7.
fn checks(table: &Table, cells: &[(&str, &str, f64)]) {
    for (row, column, expected) in cells {
        let actual = value(table, row, column);
        assert!(
            (actual - expected).abs() <= 1e-7 * expected.abs().max(1e-3),
            "{} / {row} / {column}: {actual} != {expected}",
            table.title
        );
    }
}

#[test]
fn efron_estimates_design_errors_effects_and_tests() {
    let (_, tables) = run(&program(
        "efron",
        "CSCOXREG t BY grp WITH x /VARIABLES STATUS=st(1) /PLAN FILE='wr.json'\n\
         \x20 /STATISTICS PARAMETER EXP SE TTEST CINTERVAL DEFF\n\
         \x20 /CUSTOM LABEL='first two groups' LMATRIX=grp 1 -1 0\n\
         \x20 /CUSTOM LMATRIX=ALL 1 -1 0 0\n\
         \x20 /PRINT SAMPLEINFO EVENTINFO RISKINFO HISTORY COVB.",
    ));
    let b = [0.0241154681060214, -0.368651602046722, 0.240733208722236];
    let se = [0.379497695150453, 0.3728531275513, 0.0656135401176958];
    let estimates = find(&tables, "Parameter Estimates");
    let t = 2.30600413520417; // the 97.5th percentile of t on 8 df
    for (k, name) in ["[grp=1]", "[grp=2]", "x"].iter().enumerate() {
        checks(
            estimates,
            &[
                (name, "Estimate", b[k]),
                (name, "Std. Error", se[k]),
                (name, "Hypothesis Test / t", b[k] / se[k]),
                (name, "Hypothesis Test / df", 8.0),
                (name, "Exp(B)", b[k].exp()),
                (name, "95% Confidence Interval / Lower", b[k] - t * se[k]),
            ],
        );
    }
    checks(
        estimates,
        &[
            ("[grp=1]", "Hypothesis Test / Sig.", 0.950891015800548),
            ("x", "Hypothesis Test / Sig.", 0.00631899104150526),
            ("[grp=1]", "Design Effect", 1.8802203074937),
            ("[grp=2]", "Design Effect", 1.28229768741982),
            ("x", "Design Effect", 0.717562544298403),
        ],
    );
    assert!(row_keys(estimates).contains(&"[grp=3] (redundant)".to_string()));
    checks(
        find(&tables, "Tests of Model Effects"),
        &[
            ("grp", "Wald F", 1.39642362522418),
            ("grp", "df2", 7.0),
            ("grp", "Sig.", 0.308788920075407),
            ("x", "Wald F", 13.461240034064),
            ("(Model)", "Wald F", 3.85993343832125),
            ("(Model)", "Sig.", 0.0749442741356198),
        ],
    );
    checks(
        find(&tables, "Model Summary"),
        &[
            (
                "Baseline Only",
                "-2 Log Pseudo Likelihood",
                35768.8806793724,
            ),
            ("Model", "-2 Log Pseudo Likelihood", 35413.1412063143),
        ],
    );
    checks(
        find(&tables, "Covariances of Parameter Estimates"),
        &[
            ("x", "[grp=1]", 0.00571251114291325),
            ("x", "x", se[2] * se[2]),
        ],
    );
    // b1 - b2 by effect and by ALL: one row, so F is the squared t
    for (k, custom) in tables
        .iter()
        .filter(|t| t.title == "Custom Hypothesis Tests")
        .enumerate()
    {
        checks(
            custom,
            &[
                ("Contrast 1", "Estimate", 0.392767070152743),
                ("Contrast 1", "Std. Error", 0.230331169716331),
                ("Contrast 1", "Wald F", 2.90780120351818),
                ("Overall Test", "Sig.", 0.12654734874463),
            ],
        );
        assert_eq!(
            k == 0,
            custom
                .caption
                .as_ref()
                .unwrap()
                .starts_with("first two groups")
        );
    }
    // the counts: 82 valid records (no x, a negative time), 65 events
    checks(
        find(&tables, "Sample Design Information"),
        &[
            ("N of Unweighted Cases / Valid", "Value", 82.0),
            ("N of Unweighted Cases / Invalid", "Value", 2.0),
            ("Population Size", "Value", 3067.0),
            ("Events / Unweighted Count", "Value", 65.0),
            ("Events / Population Size", "Value", 2455.5),
            ("Censored / Population Size", "Value", 611.5),
            ("Sampling Design Degrees of Freedom", "Value", 8.0),
        ],
    );
    checks(
        find(&tables, "Event and Censoring Summary"),
        &[
            ("Total", "Events / Unweighted Count", 65.0),
            ("Total", "Events / Population Size", 2455.5),
        ],
    );
    let risk = find(&tables, "Risk and Event Information");
    let at = |time: f64| {
        risk.rows
            .iter()
            .position(|r| r[0] == Cell::Number(time))
            .unwrap()
    };
    assert_eq!(
        risk.rows[at(1.0)][1..],
        [
            Cell::Number(175.0),
            Cell::Number(4.0),
            Cell::Number(3067.0),
            Cell::Number(82.0)
        ]
    );
    assert_eq!(
        risk.rows[at(7.0)][1..],
        [
            Cell::Number(420.5),
            Cell::Number(11.0),
            Cell::Number(2307.0),
            Cell::Number(63.0)
        ]
    );
    checks(
        find(&tables, "Iteration History"),
        &[
            ("0", "-2 Log Pseudo Likelihood", 35768.8806793724),
            ("0", "Parameter / x", 0.0),
        ],
    );
    let history = find(&tables, "Iteration History");
    match history.rows.last().unwrap()[5] {
        Cell::Number(v) => assert!((v - b[2]).abs() < 1e-7),
        ref other => panic!("{other:?}"),
    }
}

#[test]
fn two_times_breslow_and_baseline_strata() {
    // R: coxph(Surv(t0, t, st == 1) ~ g + x + strata(bs), ties = "breslow") on the 81 records
    // with valid values and a start below the end, the variance as in the file comment
    let (_, tables) = run(&program(
        "breslow",
        "CSCOXREG t0 t BY grp WITH x /VARIABLES STATUS=st(1) BASELINESTRATA=bs\n\
         \x20 /PLAN FILE='wr.json' /CRITERIA TIES=BRESLOW /STATISTICS PARAMETER SE\n\
         \x20 /PRINT EVENTINFO.",
    ));
    checks(
        find(&tables, "Parameter Estimates"),
        &[
            ("[grp=1]", "Estimate", 0.120415827778743),
            ("[grp=2]", "Estimate", -0.315046036184381),
            ("x", "Estimate", 0.206642498751928),
            ("[grp=1]", "Std. Error", 0.288007557961253),
            ("[grp=2]", "Std. Error", 0.381288643019302),
            ("x", "Std. Error", 0.0616509837389377),
        ],
    );
    checks(
        find(&tables, "Tests of Model Effects"),
        &[("grp", "Wald F", 1.06678005128527)],
    );
    checks(
        find(&tables, "Model Summary"),
        &[("Model", "-2 Log Pseudo Likelihood", 31146.5029012087)],
    );
    let events = find(&tables, "Event and Censoring Summary");
    assert_eq!(row_keys(events), ["1", "2", "Total"]);
    assert!(
        !tables
            .iter()
            .any(|t| t.title == "Sample Design Information")
    );
}

#[test]
fn domain_on_a_design_without_replacement() {
    // R: the clinics drawn without replacement (fpc=~npsu) and coxph on the records with
    // dom = 1; svycoxph on subset(design, dom == 1 & valid) agrees
    let (_, tables) = run(&program(
        "domain",
        "CSCOXREG t BY grp WITH x /VARIABLES STATUS=st(1) /PLAN FILE='wor.json'\n\
         \x20 /DOMAIN VARIABLE=dom(1) /STATISTICS PARAMETER SE DEFF.",
    ));
    checks(
        find(&tables, "Parameter Estimates"),
        &[
            ("[grp=1]", "Estimate", 0.0952370097572266),
            ("[grp=2]", "Estimate", -0.361037767671042),
            ("x", "Estimate", 0.228238999437321),
            ("[grp=1]", "Std. Error", 0.328301676472839),
            ("[grp=2]", "Std. Error", 0.263105572081406),
            ("x", "Std. Error", 0.067730429860084),
            ("[grp=1]", "Design Effect", 1.25213371189979),
            ("[grp=2]", "Design Effect", 0.522218741540686),
            ("x", "Design Effect", 0.717795577189036),
        ],
    );
    checks(
        find(&tables, "Tests of Model Effects"),
        &[("(Model)", "Wald F", 3.96244911280851)],
    );
}

#[test]
fn baseline_only_model_and_errors() {
    let (_, tables) = run(&program(
        "baseline",
        "CSCOXREG t /VARIABLES STATUS=st(1) /PLAN FILE='wr.json' /STATISTICS PARAMETER.",
    ));
    assert!(tables.iter().any(|t| t.title == "Model Summary"));
    assert!(!tables.iter().any(|t| t.title == "Tests of Model Effects"));
    let base = "CSCOXREG t WITH x /VARIABLES STATUS=st(1) /PLAN FILE='wr.json'";
    for (extra, before, expected) in [
        (
            "",
            "TIME PROGRAM.\nCOMPUTE st = st * 1.\n",
            "only factors and covariates",
        ),
        ("/VARIABLES STATUS=st(9)", "", "has the event"),
        (
            "/VARIABLES STATUS=st('1')",
            "",
            "event values must be numbers",
        ),
        ("/DOMAIN VARIABLE=x(1)", "", "cannot be the DOMAIN variable"),
        ("/DOMAIN VARIABLE=w(1)", "", "plan variable"),
    ] {
        let command = if extra.starts_with("/VARIABLES") {
            format!("CSCOXREG t WITH x {extra} /PLAN FILE='wr.json'.")
        } else {
            format!("{base} {extra}.")
        };
        let error = run_error(&program("errors", &format!("{before}{command}")));
        assert!(error.contains(expected), "{error}");
    }
    let error = run_error(&program(
        "errors",
        "CSCOXREG t WITH st /VARIABLES STATUS=st(1) /PLAN FILE='wr.json'.",
    ));
    assert!(error.contains("cannot be the STATUS variable"), "{error}");
}

// The baseline functions, their design-based errors and the saved values below come from an R
// script used as a black box: coxph with the weights for the estimates, survfit on it at the
// reference values (grp = 3, x at its weighted mean) for the Efron, Breslow and product-limit
// baselines (they agree to 1e-11), and the manual's influence terms of the cumulative hazard
// (v_i less A' J^-1 U_i with coxph's score residuals) totalled by svytotal on the design for
// the standard errors. Schoenfeld residuals are coxph's times the weight; Breslow martingale
// residuals are coxph's.

/// One numeric variable of the active dataset, None for system-missing.
fn column(engine: &Engine, name: &str) -> Vec<Option<f64>> {
    let data = engine.dataset().unwrap();
    let values = data.variable(name).unwrap().numeric().unwrap();
    (0..data.case_count()).map(|k| values.get(k)).collect()
}

fn near(actual: f64, expected: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= 1e-7 * expected.abs().max(1e-3),
        "{what}: {actual} != {expected}"
    );
}

/// Saved values of 1-based data cases: (variable, case, expected).
fn saved(engine: &Engine, cells: &[(&str, usize, f64)]) {
    for (name, case, expected) in cells {
        let actual = column(engine, name)[case - 1].unwrap_or(f64::NAN);
        near(actual, *expected, &format!("{name} of case {case}"));
    }
}

/// The numbers of the Baseline Functions row of a stratum (None: no strata) at `time`.
fn baseline_row(table: &Table, stratum: Option<&str>, time: f64) -> Vec<f64> {
    let mut current = String::new();
    for row in &table.rows {
        if let (Some(_), Cell::Text(t)) = (stratum, &row[0])
            && !t.is_empty()
        {
            current = t.clone();
        }
        let cells = &row[table.row_headers..];
        if cells[0] == Cell::Number(time) && stratum.is_none_or(|s| s == current) {
            return cells
                .iter()
                .map(|c| match c {
                    Cell::Number(n) => *n,
                    other => panic!("{other:?}"),
                })
                .collect();
        }
    }
    panic!("no baseline row at {time}");
}

/// Time, H, its error, S, its error and the interval in a Baseline Functions row.
fn baseline_checks(table: &Table, stratum: Option<&str>, rows: &[[f64; 7]]) {
    for expected in rows {
        let actual = baseline_row(table, stratum, expected[0]);
        for (k, (a, e)) in actual.iter().zip(expected).enumerate() {
            near(
                *a,
                *e,
                &format!("{stratum:?} at {} column {k}", expected[0]),
            );
        }
    }
}

#[test]
fn efron_baseline_pattern_plots_residuals_and_files() {
    use statscraft_engine::plot::Plot;
    let (mut engine, tables) = run(&program(
        "survival",
        "CSCOXREG t BY grp WITH x /VARIABLES STATUS=st(1) /PLAN FILE='wr.json'\n\
         \x20 /PRINT BASELINE /PATTERN grp(1) x(6) /PLOT SURVIVAL CUMHAZARD CI=YES\n\
         \x20 /SAVE MARTINGALE DEVIANCE COXSNELL SCORE DFBETA(df:2) SCHOENFELD XBETA SURVIVAL\n\
         \x20   LCL_SURVIVAL UCL_SURVIVAL CUMHAZARD LCL_CUMHAZARD UCL_CUMHAZARD AGGSCORE\n\
         \x20 /OUTFILE COVB='cox_covb.sav' MODEL='cox.json' SURVIVAL='cox_surv.sav'.",
    ));
    // time, cumulative hazard and its error, survival and its error, the 95% log interval
    baseline_checks(
        find(&tables, "Baseline Functions"),
        None,
        &[
            [
                1.0,
                0.0584168189170628,
                0.00937322378070325,
                0.943256698250165,
                0.00884135611534608,
                0.926086163113281,
                0.960745591752211,
            ],
            [
                11.0,
                0.883385222148048,
                0.207697005808621,
                0.413381153352643,
                0.0858580278090583,
                0.275143121954125,
                0.621073050031223,
            ],
            [
                28.0,
                3.20857500169575,
                1.05509459340199,
                0.0404141623714552,
                0.0426407642149925,
                0.00511017103313112,
                0.31961836689947,
            ],
        ],
    );
    saved(
        &engine,
        &[
            ("Resid_Martingale", 1, 0.836377983260491),
            ("Resid_Martingale", 5, -0.843372098012309),
            ("Resid_Deviance", 1, 1.39557751770388),
            ("Resid_Deviance", 5, -1.29874716401023),
            ("Resid_CoxSnell", 2, 2.06834522285829),
            ("Resid_Score_1", 1, 15.0699336818655),
            ("Resid_Score_3", 1, -0.993274622549517),
            ("Resid_Score_2", 2, -29.0311330741417),
            ("df_1", 1, 0.0278748675657013),
            ("df_2", 2, -0.0661265477123261),
            ("Resid_Schoenfeld_1", 1, 17.6982444676509),
            ("Resid_Schoenfeld_2", 1, -6.00620570737284),
            ("Resid_Schoenfeld_3", 2, 30.6104042180642),
            ("XBETA", 11, -1.01683773647407),
            ("Survival", 1, 0.84906289279505),
            ("LCL_Survival", 1, 0.776006842958098),
            ("UCL_Survival", 5, 0.638259056105203),
            ("CumHazard", 5, 0.843372098012309),
            ("LCL_CumHazard", 1, 0.0736500928861903),
            ("UCL_CumHazard", 5, 1.23773316202574),
        ],
    );
    // a censored record has no Schoenfeld residual, a record left out nothing; DFBETA stops at 2
    assert_eq!(column(&engine, "Resid_Schoenfeld_1")[4], None);
    assert_eq!(column(&engine, "Survival")[5], None);
    assert!(engine.dataset().unwrap().variable("df_3").is_err());
    assert!(engine.warnings().iter().any(|w| w.contains("VARIABLES ID")));

    let charts = engine.take_charts();
    let titles: Vec<&str> = charts.iter().map(|c| c.title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "Survival Function at the reference values",
            "Survival Function for Pattern 1",
            "Hazard Function at the reference values",
            "Hazard Function for Pattern 1",
        ]
    );
    let Some(Plot::Points { groups, .. }) = &charts[1].plot else {
        panic!("{:?}", charts[1].plot);
    };
    let names: Vec<&str> = groups.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(names, ["Pattern 1", "Pattern 1 lower", "Pattern 1 upper"]);
    // a step at the first event time: 1 before, the pattern's survival after
    let at1: Vec<f64> = groups[0]
        .points
        .iter()
        .filter(|p| p.x == 1.0)
        .map(|p| p.y)
        .collect();
    assert_eq!(at1[0], 1.0);
    near(at1[1], 0.939030679150369, "pattern survival at 1");
    near(
        groups[1].points[2].y,
        0.895841754085925,
        "pattern lower limit at 1",
    );

    let folder =
        std::env::temp_dir().join(format!("statscraft-cscox-survival-{}", std::process::id()));
    let model: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(folder.join("cox.json")).unwrap()).unwrap();
    assert_eq!(model["format"], "statscraft-cs-model");
    assert_eq!(model["command"], "CSCOXREG");
    assert_eq!(model["parameters"][2]["redundant"], true);
    near(
        model["baseline"][0]["cumulative_hazard"][0]
            .as_f64()
            .unwrap(),
        0.0584168189170628,
        "model file hazard",
    );
    let get = |engine: &mut Engine, what: &str| {
        let (commands, errors) = parse(what);
        assert!(errors.is_empty(), "{errors:?}");
        engine.execute(&commands[0].command).unwrap();
    };
    get(&mut engine, "GET FILE='cox_covb.sav'.");
    near(
        column(&engine, "P4")[0].unwrap(),
        0.240733208722236,
        "EST of x",
    );
    near(
        column(&engine, "P4")[1].unwrap(),
        0.0656135401176958,
        "SE of x",
    );
    assert_eq!(column(&engine, "P3")[..2], [Some(0.0), None]);
    get(&mut engine, "GET FILE='cox_surv.sav'.");
    let data = engine.dataset().unwrap();
    let patterns: Vec<String> = data
        .variable("Pattern")
        .unwrap()
        .string()
        .unwrap()
        .iter()
        .map(|s| s.trim_end().to_string())
        .collect();
    let times = column(&engine, "Time");
    let row = (0..patterns.len())
        .find(|&r| patterns[r] == "Pattern 1" && times[r] == Some(1.0))
        .unwrap();
    for (name, expected) in [
        ("Survival", 0.939030679150369),
        ("SE_Survival", 0.939030679150369 * 0.0240230777524759),
        ("LCL_Survival", 0.895841754085925),
        ("UCL_Survival", 0.984301761291902),
        ("CumHazard", 0.0629071281559799),
    ] {
        near(column(&engine, name)[row].unwrap(), expected, name);
    }
}

#[test]
fn breslow_baseline_per_stratum_with_log_minus_log_intervals() {
    let (engine, tables) = run(&program(
        "lml",
        "CSCOXREG t0 t BY grp WITH x /VARIABLES STATUS=st(1) BASELINESTRATA=bs\n\
         \x20 /PLAN FILE='wr.json' /CRITERIA TIES=BRESLOW /SURVIVALMETHOD CI=LML\n\
         \x20 /PRINT BASELINE /SAVE MARTINGALE SURVIVAL LCL_SURVIVAL UCL_SURVIVAL CUMHAZARD.",
    ));
    let table = find(&tables, "Baseline Functions");
    baseline_checks(
        table,
        Some("1"),
        &[[
            8.0,
            0.557222310489553,
            0.124074700915722,
            0.572797910914083,
            0.0710697294818152,
            0.422270302282582,
            0.697565277885087,
        ]],
    );
    baseline_checks(
        table,
        Some("2"),
        &[
            [
                1.0,
                0.0932065802383863,
                0.05523250347341,
                0.911005285167066,
                0.0503171025772848,
                0.742486344078014,
                0.971244566695646,
            ],
            [
                28.0,
                4.39438127983927,
                1.49133227967619,
                0.0123465170016878,
                0.0184127593461879,
                0.000194285401046815,
                0.104396713688238,
            ],
        ],
    );
    // case 2 enters at 15: its martingale residual counts the hazard from there, its survival
    // and cumulative hazard are from time 0
    saved(
        &engine,
        &[
            ("Resid_Martingale", 1, 0.856568430757883),
            ("Resid_Martingale", 2, -0.626789743369594),
            ("Resid_Martingale", 5, -1.24769352901398),
            ("Survival", 2, 0.0488272684234298),
            ("LCL_Survival", 2, 0.0110487913615633),
            ("UCL_Survival", 1, 0.962507308409967),
            ("CumHazard", 2, 3.01946634301334),
        ],
    );
}

#[test]
fn product_limit_baseline_in_a_domain_with_original_intervals() {
    let (engine, tables) = run(&program(
        "productlimit",
        "CSCOXREG t BY grp WITH x /VARIABLES STATUS=st(1) /PLAN FILE='wor.json'\n\
         \x20 /DOMAIN VARIABLE=dom(1) /SURVIVALMETHOD BASELINE=PRODUCTLIMIT CI=ORIGINAL\n\
         \x20 /PRINT BASELINE /SAVE MARTINGALE(m) SURVIVAL(s) LCL_SURVIVAL(lo) UCL_SURVIVAL(hi).",
    ));
    baseline_checks(
        find(&tables, "Baseline Functions"),
        None,
        &[
            [
                2.0,
                0.105151651888848,
                0.0209687306229343,
                0.90018799702428,
                0.0188757996196009,
                0.863192109590468,
                0.937183884458093,
            ],
            // the lower limit is cut at 0
            [
                28.0,
                4.32221344559503,
                0.861948109960577,
                0.0132704775301124,
                0.0114384630253547,
                0.0,
                0.0356894530983007,
            ],
        ],
    );
    saved(
        &engine,
        &[
            ("m", 7, 0.435318314769816),
            ("m", 15, -0.0858524245313088),
            ("s", 7, 0.568541092984229),
            ("lo", 7, 0.470870207441959),
            ("hi", 2, 0.115819989046482),
        ],
    );
    // case 8 is outside the domain
    assert_eq!(column(&engine, "m")[7], None);
}

#[test]
fn pattern_lines_and_errors() {
    let base = "CSCOXREG t BY grp WITH x /VARIABLES STATUS=st(1) /PLAN FILE='wr.json' /PLOT OMS";
    for (pattern, expected) in [
        ("/PATTERN w(1)", "not a factor or covariate"),
        ("/PATTERN grp(7)", "not a level of grp"),
        ("/PATTERN BY x", "must be a factor"),
        ("/PATTERN x('a')", "takes a number"),
    ] {
        let error = run_error(&program("patterns", &format!("{base} {pattern}.")));
        assert!(error.contains(expected), "{error}");
    }
    let (mut engine, _) = run(&program("by", &format!("{base} LML /PATTERN x(5) BY grp.")));
    let charts = engine.take_charts();
    assert_eq!(charts.len(), 4);
    let Some(statscraft_engine::plot::Plot::Points { groups, .. }) = &charts[3].plot else {
        panic!("{:?}", charts[3].plot);
    };
    let names: Vec<&str> = groups.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Pattern 1, grp = 1",
            "Pattern 1, grp = 2",
            "Pattern 1, grp = 3"
        ]
    );
}

/// 60 invented records of 30 subjects (id) in 3 strata (str) with a weight per subject: each
/// subject has 1 to 3 records with consecutive intervals (t0, t], a factor grp, a covariate x
/// that changes from record to record, and a status st (1 is the event) that only a subject's
/// last record can have. The plan has strata and no clusters, so the subjects are the units.
const SUBJECTS: &str = "
101,1,13.5,0,4,0,2,5 101,1,13.5,4,7,1,2,5.1 102,1,15,0,4,0,3,5.5 102,1,15,4,6,0,3,5
102,1,15,6,9,0,3,6.3 103,1,12,0,6,1,1,2.1 104,1,13.5,0,5,0,2,4.9 104,1,13.5,5,7,1,2,4.4
105,1,15,0,2,0,3,4.1 105,1,15,2,8,0,3,5 105,1,15,8,14,1,3,3.5 106,1,12,0,6,1,1,6.7
107,1,13.5,0,2,0,2,6.1 107,1,13.5,2,8,1,2,4.6 108,1,15,0,6,0,3,3.7 108,1,15,6,11,0,3,2.6
108,1,15,11,16,1,3,3.7 109,1,12,0,5,1,1,4.1 110,1,13.5,0,5,0,2,3.9 110,1,13.5,5,8,1,2,4.2
111,2,21.5,0,6,0,3,3 111,2,21.5,6,9,0,3,3.5 111,2,21.5,9,16,1,3,2.4 112,2,23,0,7,1,1,4.6
113,2,20,0,3,0,2,6.9 113,2,20,3,7,0,2,6.8 114,2,21.5,0,5,0,3,7.5 114,2,21.5,5,11,0,3,7.8
114,2,21.5,11,18,0,3,6.6 115,2,23,0,5,0,1,5.4 116,2,20,0,4,0,2,2.5 116,2,20,4,11,1,2,2
117,2,21.5,0,3,0,3,4.6 117,2,21.5,3,9,0,3,5.2 117,2,21.5,9,11,1,3,4.3 118,2,23,0,7,0,1,4.3
119,2,20,0,5,0,2,6.1 119,2,20,5,7,1,2,6.7 120,2,21.5,0,4,0,3,5.5 120,2,21.5,4,6,0,3,6.9
120,2,21.5,6,11,1,3,5.8 121,3,10.5,0,6,0,1,7.6 122,3,12,0,6,0,2,4.2 122,3,12,6,9,1,2,4.1
123,3,9,0,5,0,3,4 123,3,9,5,12,0,3,3.8 123,3,9,12,17,0,3,3.2 124,3,10.5,0,4,1,1,5.1
125,3,12,0,4,0,2,2.4 125,3,12,4,11,0,2,2.1 126,3,9,0,7,0,3,3.7 126,3,9,7,14,0,3,3.4
126,3,9,14,19,0,3,3.6 127,3,10.5,0,2,1,1,2.1 128,3,12,0,5,0,2,2.5 128,3,12,5,9,1,2,3.9
129,3,9,0,2,0,3,6.5 129,3,9,2,4,0,3,7 129,3,9,4,6,1,3,7.7 130,3,10.5,0,3,0,1,6.6
";

/// The subjects' data and a plan with strata and no clusters, then `commands`.
fn subjects(folder: &str, commands: &str) -> String {
    let dir =
        std::env::temp_dir().join(format!("statscraft-cscox-{folder}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let records: Vec<&str> = SUBJECTS.split_whitespace().collect();
    format!(
        "DATA LIST LIST(\",\") /id str w t0 t st grp x.\n\
         BEGIN DATA\n{}\nEND DATA.\nCD '{}'.\n\
         CSPLAN ANALYSIS /PLAN FILE='s.json' /PLANVARS ANALYSISWEIGHT=w\n\
         \x20 /DESIGN STRATA=str /ESTIMATOR TYPE=WR.\n\
         {commands}\n",
        records.join("\n"),
        dir.display()
    )
}

// The references: coxph with the weights on the 60 records (grp releveled to its last level),
// and svycoxph on svydesign(ids=~id, strata=~str, weights=~w), which agrees to 1e-14 with the
// manual's J^-1 V J^-1, V the design covariance of the subjects' totals of w U. The design
// effects divide by J^-1 S J^-1, S = (1 - n/N) N/(n - 1) sum over the 30 subjects of w U U'
// with n = 30 and N their summed weights. Martingale
// residuals are coxph's (Breslow ties); the aggregated ones sum a subject's records, the
// aggregated deviance comes from that sum and the subject's events, and the aggregated score
// and DFBETA sum w U and w J^-1 U.
#[test]
fn subjects_as_units_with_one_or_two_times_and_aggregated_residuals() {
    let b = [3.42032366801177, 2.03439957958917, 0.129124170743974];
    let se = [0.774089524097684, 0.579964861346988, 0.182697420224556];
    let deff = [0.936653780523957, 0.919169743144461, 1.29353604341029];
    // the one-time form takes each start from the subject's previous record
    for times in ["t0 t", "t"] {
        let (_, tables) = run(&subjects(
            "ids",
            &format!(
                "CSCOXREG {times} BY grp WITH x /VARIABLES STATUS=st(1) ID=id /PLAN FILE='s.json'\n\
                 \x20 /STATISTICS PARAMETER SE DEFF."
            ),
        ));
        let estimates = find(&tables, "Parameter Estimates");
        for (k, name) in ["[grp=1]", "[grp=2]", "x"].iter().enumerate() {
            checks(
                estimates,
                &[
                    (name, "Estimate", b[k]),
                    (name, "Std. Error", se[k]),
                    (name, "Design Effect", deff[k]),
                ],
            );
        }
        checks(
            find(&tables, "Sample Design Information"),
            &[("Sampling Design Degrees of Freedom", "Value", 27.0)],
        );
    }
    let (engine, tables) = run(&subjects(
        "aggregated",
        "CSCOXREG t0 t BY grp WITH x /VARIABLES STATUS=st(1) ID=id /PLAN FILE='s.json'\n\
         \x20 /CRITERIA TIES=BRESLOW /STATISTICS PARAMETER SE\n\
         \x20 /SAVE MARTINGALE AGGMARTINGALE AGGDEVIANCE AGGCOXSNELL AGGSCORE AGGDFBETA.",
    ));
    checks(
        find(&tables, "Parameter Estimates"),
        &[
            ("[grp=1]", "Estimate", 3.22443589843098),
            ("[grp=1]", "Std. Error", 0.676011670903849),
            ("x", "Std. Error", 0.172260049731465),
        ],
    );
    saved(
        &engine,
        &[
            ("Resid_Martingale", 1, -0.0310224472826235),
            ("Resid_Martingale", 2, 0.693224658716225),
            ("Resid_Martingale", 3, -0.00492788608392747),
        ],
    );
    // (case, martingale, deviance, Cox-Snell, score, DFBETA) of a subject's last record
    type Aggregated = (usize, f64, f64, f64, [f64; 3], [f64; 3]);
    let rows: [Aggregated; 3] = [
        (
            2,
            0.662202211433602,
            0.919897396205623,
            0.337797788566398,
            [-4.07315466245363, 4.73384388382106, 4.31187964703694],
            [-0.0774740947889029, 0.0589564513801266, 0.0100594443520802],
        ),
        (
            5,
            -0.171608654301778,
            -0.585847513098379,
            0.171608654301778,
            [0.463605372851522, 1.5364801459814, -5.15108267951745],
            [0.0622477398107051, 0.0555648264729451, -0.00153982995962081],
        ),
        (
            18,
            0.828738117049916,
            1.36808135555433,
            0.171261882950084,
            [2.79087910285308, -2.37187633597585, -9.38095931015091],
            [0.0613345180000545, -0.029436830819399, -0.0142052069420278],
        ),
    ];
    for (case, m, dev, cs, score, dfbeta) in rows {
        saved(
            &engine,
            &[
                ("AggResid_Martingale", case, m),
                ("AggResid_Deviance", case, dev),
                ("AggResid_CoxSnell", case, cs),
            ],
        );
        for k in 0..3 {
            saved(
                &engine,
                &[
                    (&format!("AggResid_Score_{}", k + 1), case, score[k]),
                    (&format!("AggResid_DFBETA_{}", k + 1), case, dfbeta[k]),
                ],
            );
        }
    }
    // on the subject's other records the aggregated residuals are missing
    assert_eq!(column(&engine, "AggResid_Martingale")[0], None);
    assert_eq!(column(&engine, "AggResid_Score_1")[3], None);
}

// The alternative model on the subjects' records split at the event times with the predictors
// times t, fitted by svycoxph with ids=~id; Wald F on 3 and 27 - 3 + 1 degrees of freedom.
#[test]
fn proportional_hazards_test_with_subjects() {
    let (_, tables) = run(&subjects(
        "subjects-ph",
        "CSCOXREG t0 t BY grp WITH x /VARIABLES STATUS=st(1) ID=id /PLAN FILE='s.json'\n\
         \x20 /TESTASSUMPTIONS PROPHAZARD=IDENTITY PARAMETER.",
    ));
    checks(
        find(&tables, "Overall Test of Proportional Hazards"),
        &[
            ("Identity", "Wald F", 0.611348801137133),
            ("Identity", "df2", 25.0),
            ("Identity", "Sig.", 0.613955744512214),
        ],
    );
    checks(
        find(&tables, "Parameter Estimates of the Alternative Model"),
        &[
            ("[grp=1]", "Std. Error", 11.8321471178769),
            ("x * T", "Std. Error", 0.0422954343768367),
        ],
    );
}

// The references for the 81 valid records: survSplit at the event times (keeping the pieces that
// end at one), then svycoxph on svydesign(ids=~psu, strata=~str, weights=~w, nest=TRUE) of the
// pieces. The time-dependent covariate is x ln(t) at each piece's end; record scores are
// coxph's unweighted score residuals summed over a record's pieces times its weight.
#[test]
fn covariates_from_a_time_program() {
    let (engine, tables) = run(&program(
        "program",
        "TIME PROGRAM.\nCOMPUTE xt = x * LN(T_).\n\
         CSCOXREG t0 t BY grp WITH x xt /VARIABLES STATUS=st(1) /PLAN FILE='wr.json'\n\
         \x20 /STATISTICS PARAMETER SE /SAVE SCORE DFBETA MARTINGALE XBETA /PRINT BASELINE.",
    ));
    let b = [
        0.0722124527201917,
        -0.223632219461664,
        0.200494908885687,
        0.0150896126829603,
    ];
    let se = [
        0.375174187646778,
        0.41405856000685,
        0.0894507796560857,
        0.0696400685206921,
    ];
    let estimates = find(&tables, "Parameter Estimates");
    for (k, name) in ["[grp=1]", "[grp=2]", "x", "xt"].iter().enumerate() {
        checks(
            estimates,
            &[(name, "Estimate", b[k]), (name, "Std. Error", se[k])],
        );
    }
    assert!(!tables.iter().any(|t| t.title == "Baseline Functions"));
    assert!(
        engine
            .warnings()
            .iter()
            .any(|w| w.contains("no baseline functions"))
    );
    let scores = [
        (
            2,
            [
                6.20691056698835,
                -11.44381784231467,
                16.284265965516,
                52.6863324883265,
            ],
        ),
        (
            10,
            [
                -4.95577668129684,
                7.08506022302379,
                -10.1329187560551,
                -56.22706937242,
            ],
        ),
    ];
    for (case, values) in scores {
        for (k, v) in values.iter().enumerate() {
            saved(&engine, &[(&format!("Resid_Score_{}", k + 1), case, *v)]);
        }
    }
    let dfbeta = [
        0.00110580403753345,
        -0.0278204971214205,
        -0.00677393361309823,
        0.0059240885664982,
    ];
    for (k, v) in dfbeta.iter().enumerate() {
        saved(&engine, &[(&format!("Resid_DFBETA_{}", k + 1), 2, *v)]);
    }
    let data = engine.dataset().unwrap();
    assert!(data.variable("Resid_Martingale").is_err());
    assert!(data.variable("XBETA").is_ok());
    // a program that changes no model variable is not used
    let (engine, tables) = run(&program(
        "unused",
        "TIME PROGRAM.\nCOMPUTE other = T_.\n\
         CSCOXREG t BY grp WITH x /VARIABLES STATUS=st(1) /PLAN FILE='wr.json' /STATISTICS PARAMETER.",
    ));
    checks(
        find(&tables, "Parameter Estimates"),
        &[("x", "Estimate", 0.240733208722236)],
    );
    assert!(engine.warnings().iter().any(|w| w.contains("not used")));
}

// The alternative models: the same split pieces with the predictors times g at each piece's end
// (1 minus the weighted Kaplan-Meier survival of the valid records, the rank of the event time
// plus 1, its log or the time itself), fitted by svycoxph; the Wald F is the manual's
// (df - r + 1)/(r df) X2 on r = 3 and 8 - 3 + 1 = 6 degrees of freedom.
#[test]
fn proportional_hazards_tests_and_estimable_functions() {
    let (_, tables) = run(&program(
        "prophazard",
        "CSCOXREG t0 t BY grp WITH x /VARIABLES STATUS=st(1) /PLAN FILE='wr.json'\n\
         \x20 /TESTASSUMPTIONS PROPHAZARD=KM PARAMETER COVB /PRINT GEF.",
    ));
    checks(
        find(&tables, "Overall Test of Proportional Hazards"),
        &[
            ("Kaplan-Meier", "Wald F", 0.106911531666538),
            ("Kaplan-Meier", "df1", 3.0),
            ("Kaplan-Meier", "df2", 6.0),
            ("Kaplan-Meier", "Sig.", 0.952981253164657),
        ],
    );
    let names = [
        "[grp=1]",
        "[grp=2]",
        "x",
        "[grp=1] * (1 - KM)",
        "[grp=2] * (1 - KM)",
        "x * (1 - KM)",
    ];
    let b = [
        -0.251330409961638,
        -0.61717509417245,
        0.222151414902697,
        0.711821682456732,
        0.831108729207248,
        0.000499386176140916,
    ];
    let se = [
        0.915532289854753,
        0.637619500339405,
        0.0623315379240695,
        1.74375650021995,
        1.62686384755964,
        0.241207456127783,
    ];
    let estimates = find(&tables, "Parameter Estimates of the Alternative Model");
    for k in 0..6 {
        checks(
            estimates,
            &[
                (names[k], "Estimate", b[k]),
                (names[k], "Std. Error", se[k]),
            ],
        );
    }
    let covb = find(&tables, "Covariances of the Alternative Model Parameters");
    checks(
        covb,
        &[
            (names[3], names[4], 1.9004583154805),
            (names[0], names[5], -0.0214843082612111),
        ],
    );
    // the estimable functions of grp contrast each level with the last
    let gef = find(&tables, "General Estimable Function");
    let expected = [
        ("[grp=1]", [1.0, 0.0, 0.0]),
        ("[grp=2]", [0.0, 1.0, 0.0]),
        ("[grp=3]", [-1.0, -1.0, 0.0]),
        ("x", [0.0, 0.0, 1.0]),
    ];
    for (row, values) in expected {
        for (k, v) in values.iter().enumerate() {
            assert_eq!(value(gef, row, &format!("L{}", k + 1)), *v, "{row} L{k}");
        }
    }
    for (function, label, f, sig) in [
        ("RANK", "Rank", 0.068349758094751, 0.974748534647914),
        ("LOG", "Log", 1.074552846779507, 0.427840730808268),
        ("IDENTITY", "Identity", 0.105262521435373, 0.95396882472438),
    ] {
        let (_, tables) = run(&program(
            "prophazard",
            &format!(
                "CSCOXREG t0 t BY grp WITH x /VARIABLES STATUS=st(1) /PLAN FILE='wr.json'\n\
                 \x20 /TESTASSUMPTIONS PROPHAZARD({function})."
            ),
        ));
        checks(
            find(&tables, "Overall Test of Proportional Hazards"),
            &[(label, "Wald F", f), (label, "Sig.", sig)],
        );
    }
}

// The path's cumulative hazard and survival are survfit's on the weighted coxph (Efron) with a
// counting-process newdata for one id; its standard error is the manual's influence of each
// record, sum over the event times of r (v0 terms) less G'J^-1 U with r = exp(z'b) of the step
// in force, totalled by svytotal on the design.
#[test]
fn piecewise_pattern_path() {
    let (mut engine, _) = run(&program(
        "path",
        "CSCOXREG t0 t BY grp WITH x /VARIABLES STATUS=st(1) /PLAN FILE='wr.json'\n\
         \x20 /PATTERN t(5 12 40) x(4 6 8) grp(1) /OUTFILE SURVIVAL='path.sav'.",
    ));
    let (commands, _) = parse("GET FILE='path.sav'.");
    engine.execute(&commands[0].command).unwrap();
    let data = engine.dataset().unwrap();
    let patterns: Vec<String> = data
        .variable("Pattern")
        .unwrap()
        .string()
        .unwrap()
        .iter()
        .map(|s| s.trim_end().to_string())
        .collect();
    let times = column(&engine, "Time");
    let expected = [
        (
            1.0,
            0.0574120958938587,
            0.944204886225069,
            0.020975555236835,
        ),
        (4.0, 0.18214268367276, 0.833482407599954, 0.0482446125826713),
        (
            5.0,
            0.209135273881993,
            0.811285482483591,
            0.0589406558992799,
        ),
        (
            6.0,
            0.279883798108209,
            0.755871570059094,
            0.0853516608310496,
        ),
    ];
    for (time, h, s, se) in expected {
        let row = (0..patterns.len())
            .find(|&r| patterns[r] == "Pattern 1" && times[r] == Some(time))
            .unwrap();
        near(column(&engine, "CumHazard")[row].unwrap(), h, "path hazard");
        near(
            column(&engine, "Survival")[row].unwrap(),
            s,
            "path survival",
        );
        near(
            column(&engine, "SE_Survival")[row].unwrap(),
            s * se,
            "path error",
        );
    }
    // the path ends at its last end time
    assert!((0..patterns.len()).all(|r| patterns[r] != "Pattern 1" || times[r] <= Some(40.0)));
}
