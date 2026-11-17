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
            "TIME PROGRAM.\nCOMPUTE T_COV_ = x * T_.\n",
            "TIME PROGRAM",
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
