//! PLUM on synthetic data. The expected numbers come from an independent fit of the same
//! cumulative link models in a general statistics package: the estimates from its ordinal
//! regression routine (refined by a general optimizer for the Cauchit link and for the scale
//! and non-parallel models, which the routine does not fit), the standard errors from a
//! numerical second derivative of the log-likelihood, and the pooled goodness-of-fit sums,
//! the multinomial constant and the pseudo R-squares written out from their formulas.

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

/// The row headers joined with " / ", an empty header taking the one above it.
fn row_keys(table: &Table) -> Vec<String> {
    let mut carried = vec![String::new(); table.row_headers];
    table
        .rows
        .iter()
        .map(|row| {
            for (level, cell) in row.iter().take(table.row_headers).enumerate() {
                if let Cell::Text(t) = cell {
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

fn cell<'t>(tables: &'t [Table], title: &str, row: &str, column: &str) -> &'t Cell {
    let table = tables
        .iter()
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

fn number(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        other => panic!("not a number: {other:?}"),
    }
}

fn assert_close(actual: f64, expected: f64, tolerance: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= tolerance * expected.abs().max(1.0),
        "{what}: {actual} against {expected}"
    );
}

fn values(engine: &Engine, name: &str) -> Vec<Option<f64>> {
    let data = engine.dataset().unwrap();
    match data.variable(name).unwrap().values() {
        Values::Numeric(v) => (0..data.case_count()).map(|k| v.get(k)).collect(),
        Values::String(_) => panic!("{name} is a string"),
    }
}

/// Parcel delivery delay class (1 on time to 4 very late) by region, service plan and
/// distance, with a frequency weight. Drawn from a logistic latent variable model. Case 7 has
/// a weight that rounds to 0, case 12 no distance, case 20 a user-missing delay.
const DATA: &str = "DATA LIST FREE /delay region (F1) plan (A5) dist w.
BEGIN DATA.
1 2 plus 16.4 1 3 1 plus 18.2 1 2 1 plus 38.6 1 2 2 plus 13.1 0.6 1 1 basic 3.6 1
3 2 basic 30.3 1 1 3 plus 15 0.4 1 1 plus 6.3 1 2 2 basic 4.5 2.2 1 1 plus 37.6 1
2 3 plus 25.7 1.4 3 1 basic . 0.6 1 3 basic 4.8 1 2 3 basic 14.4 1 2 2 plus 15.1 1
1 2 basic 10.3 1 4 2 plus 31.3 1 4 1 plus 7.1 0.6 1 1 plus 15.8 1 9 1 basic 27.4 1
2 3 plus 27.4 1 4 1 basic 24.6 1 2 3 plus 21.2 2.2 1 1 plus 17.7 1 1 2 plus 12.8 0.6
4 1 basic 16.8 1 2 3 basic 23 1 1 3 plus 2.2 2.2 3 1 basic 12.7 1 4 2 basic 17.1 1.4
4 1 basic 37.4 0.6 1 1 plus 11.1 2.2 2 3 basic 25.8 1 3 1 basic 16.1 2.2 1 2 plus 19 1
1 1 basic 12.4 2.2 2 2 basic 6.9 1 4 1 plus 9.8 1 1 3 plus 19.6 1 3 3 basic 25.3 1.4
1 1 plus 25.1 1 3 3 plus 5.5 0.6 2 2 basic 31.6 1 4 2 plus 32 2.2 1 1 plus 13.9 1.4
1 3 plus 39.4 1 2 3 basic 20.8 1.4 4 2 plus 7.4 1 2 1 basic 3.2 1 4 2 basic 23.6 1
1 3 plus 37.8 1 1 1 basic 5.7 1 3 2 basic 20.9 2.2 4 2 basic 27.1 1 1 3 plus 38 0.6
1 1 plus 2.3 2.2 3 3 basic 39.1 0.6 4 1 plus 29.3 1 1 3 plus 9.4 1 4 1 basic 23.8 1
1 3 plus 16.5 1 1 1 basic 6.7 1 1 2 basic 20.7 1 1 1 plus 5.8 2.2 2 2 plus 16.5 1
1 3 plus 27.7 2.2 4 3 basic 34.9 1 3 2 basic 22.8 1 4 1 basic 16.1 1 1 2 plus 4.1 0.6
1 3 plus 30 2.2 2 3 basic 8.8 1.4 2 3 plus 12.9 2.2 2 1 basic 14.8 2.2 1 3 plus 20.7 1
1 1 plus 20.7 1 2 2 basic 26.1 1 3 2 plus 36.4 1 1 3 basic 37.2 1.4 3 1 plus 20.7 1
3 2 basic 28.2 1 1 1 plus 35.1 1 2 1 basic 34.4 1 1 2 plus 18.1 1.4 2 2 basic 24.8 0.6
2 2 basic 7.8 2.2 2 2 basic 5.6 0.6 2 2 plus 16.4 0.6 1 3 plus 26 1 2 3 plus 11 1
END DATA.
MISSING VALUES delay (9).
WEIGHT BY w.
";

const LOCATION_ROWS: [&str; 7] = [
    "Threshold / [delay=1]",
    "Threshold / [delay=2]",
    "Threshold / [delay=3]",
    "Location / dist",
    "Location / [region=1]",
    "Location / [region=2]",
    "Location / [plan=basic]",
];

fn check_parameters(tables: &[Table], rows: &[&str], estimates: &[f64], errors: &[f64]) {
    for ((row, b), se) in rows.iter().zip(estimates).zip(errors) {
        let est = number(cell(tables, "Parameter Estimates", row, "Estimate"));
        let std = number(cell(tables, "Parameter Estimates", row, "Std. Error"));
        assert_close(est, *b, 2e-6, &format!("{row} estimate"));
        assert_close(std, *se, 2e-5, &format!("{row} standard error"));
        let wald = number(cell(tables, "Parameter Estimates", row, "Wald"));
        assert_close(wald, (est / std).powi(2), 1e-12, "Wald");
        let lower = number(cell(
            tables,
            "Parameter Estimates",
            row,
            "95% Confidence Interval / Lower Bound",
        ));
        assert_close(lower, est - 1.959963984540054 * std, 1e-9, "lower bound");
    }
}

#[test]
fn logit_model_matches_the_reference_fit() {
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        &format!("{DATA}PLUM delay BY region plan WITH dist /PRINT=FIT SUMMARY PARAMETER KERNEL."),
    );
    let n = |row: &str| number(cell(&tables, "Case Processing Summary", row, "N"));
    assert_eq!(n("Valid"), 102.0);
    assert_eq!(n("Missing"), 2.0);
    assert_eq!(n("Total"), 104.0);
    assert_eq!(n("delay / 1"), 42.0);
    assert_eq!(n("plan / plus"), 56.0);
    check_parameters(
        &tables,
        &LOCATION_ROWS,
        &[
            1.737328089,
            3.265617737,
            4.195595847,
            0.05316915186,
            0.5637787768,
            1.111502013,
            1.309316784,
        ],
        &[
            0.5827568967,
            0.654037967,
            0.7089041614,
            0.01941712072,
            0.4917549271,
            0.4788770671,
            0.399607447,
        ],
    );
    for row in ["Location / [region=3]", "Location / [plan=plus]"] {
        assert_eq!(
            number(cell(&tables, "Parameter Estimates", row, "Estimate")),
            0.0
        );
        assert_eq!(number(cell(&tables, "Parameter Estimates", row, "df")), 0.0);
    }
    let fit =
        |row: &str, column: &str| number(cell(&tables, "Model Fitting Information", row, column));
    assert_close(
        fit("Intercept Only", "-2 Log Likelihood"),
        262.8419187,
        1e-9,
        "-2LL 0",
    );
    assert_close(fit("Final", "-2 Log Likelihood"), 237.9605129, 1e-9, "-2LL");
    assert_close(
        fit("Final", "Chi-Square"),
        262.8419187 - 237.9605129,
        1e-8,
        "chi-square",
    );
    assert_eq!(fit("Final", "df"), 4.0);
    let r2 = |row: &str| number(cell(&tables, "Pseudo R-Square", row, "Value"));
    assert_close(r2("Cox and Snell"), 0.2164617126, 1e-8, "Cox and Snell");
    assert_close(r2("Nagelkerke"), 0.2342686279, 1e-8, "Nagelkerke");
    assert_close(r2("McFadden"), 0.09466300484, 1e-8, "McFadden");
    let gof = |row: &str, column: &str| number(cell(&tables, "Goodness-of-Fit", row, column));
    assert_close(gof("Pearson", "Chi-Square"), 311.0120596, 1e-8, "Pearson");
    assert_close(gof("Deviance", "Chi-Square"), 228.5962505, 1e-8, "deviance");
    // 84 pooled subpopulations, 3 cumulative logits each, 7 parameters
    assert_eq!(gof("Pearson", "df"), 245.0);
    assert!(
        engine
            .warnings()
            .iter()
            .any(|w| w.contains("249 of the 336 cells"))
    );
}

#[test]
fn every_link_matches_the_reference_fit() {
    // link, estimates, standard errors, kernel -2 log likelihood
    let cases: [(&str, [f64; 7], [f64; 7], f64); 4] = [
        (
            "PROBIT",
            [
                1.004625793,
                1.90758609,
                2.439499226,
                0.02987115212,
                0.4152588054,
                0.7179917477,
                0.715974617,
            ],
            [
                0.3332395868,
                0.3604006634,
                0.3849356202,
                0.01126795773,
                0.2884041316,
                0.2886323396,
                0.2309303836,
            ],
            238.9673668,
        ),
        (
            "CLOGLOG",
            [
                0.502264232,
                1.437492809,
                1.9243628,
                0.02720890656,
                0.6021157646,
                0.725826275,
                0.4773322646,
            ],
            [
                0.2979829537,
                0.3040348313,
                0.326147493,
                0.01158035416,
                0.2788868686,
                0.2822265695,
                0.2285232135,
            ],
            243.4087859,
        ),
        (
            "NLOGLOG",
            [
                1.59472066,
                2.702445696,
                3.47152675,
                0.03467775264,
                0.2790285952,
                0.7913045681,
                1.036631508,
            ],
            [
                0.4473468373,
                0.4964600324,
                0.5367989967,
                0.01308431367,
                0.365194672,
                0.3504377222,
                0.2846224216,
            ],
            235.7913006,
        ),
        (
            "CAUCHIT",
            [
                1.410186817,
                2.988515432,
                4.18831012,
                0.04777366814,
                -0.07745474677,
                0.7567316127,
                1.427961265,
            ],
            [
                0.5937233557,
                0.7594661457,
                0.904918822,
                0.01971661903,
                0.5142980737,
                0.4372544189,
                0.4432839444,
            ],
            236.4338858,
        ),
    ];
    for (link, estimates, errors, m2ll) in cases {
        let mut engine = Engine::default();
        let tables = run(
            &mut engine,
            &format!(
                "{DATA}PLUM delay BY region plan WITH dist /LINK={link} /PRINT=PARAMETER KERNEL."
            ),
        );
        check_parameters(&tables, &LOCATION_ROWS, &estimates, &errors);
        let final_m2ll = number(cell(
            &tables,
            "Model Fitting Information",
            "Final",
            "-2 Log Likelihood",
        ));
        assert_close(final_m2ll, m2ll, 1e-8, link);
        let caption = tables
            .iter()
            .find(|t| t.title == "Parameter Estimates")
            .unwrap()
            .caption
            .clone()
            .unwrap();
        assert!(caption.starts_with("Link function:"), "{caption}");
    }
}

#[test]
fn scale_model_matches_the_reference_fit() {
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        &format!("{DATA}PLUM delay BY region plan WITH dist /SCALE=plan /PRINT=PARAMETER KERNEL."),
    );
    let mut rows = LOCATION_ROWS.to_vec();
    rows.push("Scale / [plan=basic]");
    check_parameters(
        &tables,
        &rows,
        &[
            1.529828302,
            2.556675891,
            3.138128759,
            0.04341416862,
            0.4762506862,
            0.6186540904,
            1.127652177,
            -0.7678483924,
        ],
        &[
            0.4372580212,
            0.5587760454,
            0.6551579597,
            0.01375586553,
            0.3218430344,
            0.33446747,
            0.3143949026,
            0.2882593729,
        ],
    );
    assert_eq!(
        number(cell(
            &tables,
            "Parameter Estimates",
            "Scale / [plan=plus]",
            "df"
        )),
        0.0
    );
    let fit = |column: &str| number(cell(&tables, "Model Fitting Information", "Final", column));
    assert_close(fit("-2 Log Likelihood"), 230.6931842, 1e-8, "-2LL");
    assert_eq!(fit("df"), 5.0);
}

#[test]
fn save_adds_the_predicted_probabilities() {
    let mut engine = Engine::default();
    run(
        &mut engine,
        &format!(
            "{DATA}PLUM delay BY region plan WITH dist /SAVE=ESTPROB PREDCAT PCPROB ACPROB(actual)."
        ),
    );
    let expected = [
        (
            0,
            [0.4387718299, 0.3440422793, 0.11851849132, 0.09866739947],
        ),
        (
            1,
            [0.5512866756, 0.2986571316, 0.08493327343, 0.06512291930],
        ),
        (
            4,
            [0.4189278021, 0.3497949594, 0.12516580674, 0.10611143173],
        ),
    ];
    let est: Vec<Vec<Option<f64>>> = (1..=4)
        .map(|c| values(&engine, &format!("EST{c}_1")))
        .collect();
    for (case, probabilities) in expected {
        for (c, p) in probabilities.iter().enumerate() {
            assert_close(est[c][case].unwrap(), *p, 1e-7, "ESTPROB");
        }
    }
    let predicted = values(&engine, "PRE_1");
    let pcp = values(&engine, "PCP_1");
    let actual = values(&engine, "actual");
    // case 2 has delay 3 and is predicted on time
    assert_eq!(predicted[1], Some(1.0));
    assert_close(pcp[1].unwrap(), 0.5512866756, 1e-7, "PCPROB");
    assert_close(actual[1].unwrap(), 0.08493327343, 1e-7, "ACPROB");
    // a weight that rounds to 0 still gets predictions; no distance gets none; a missing
    // delay gets all but the probability of its category
    assert!(predicted[6].is_some());
    assert_eq!(predicted[11], None);
    assert!(est[0][19].is_some() && actual[19].is_none());
}

/// Customer rating (1 to 3) by region and plan, as counts in a weight.
const GROUPED: &str = "DATA LIST FREE /region (F1) plan (A5) rating count.
BEGIN DATA.
1 basic 1 12  1 basic 2 7  1 basic 3 3  1 plus 1 8  1 plus 2 9  1 plus 3 6
2 basic 1 5  2 basic 2 6  2 plus 1 4  2 plus 2 8  2 plus 3 9  3 basic 1 9
3 basic 2 2  3 basic 3 1  3 plus 1 3  3 plus 2 5  3 plus 3 11
END DATA.
WEIGHT BY count.
";

#[test]
fn grouped_data_fit_and_parallel_lines() {
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        &format!("{GROUPED}PLUM rating BY region plan /PRINT=FIT PARAMETER TPARALLEL."),
    );
    check_parameters(
        &tables,
        &[
            "Threshold / [rating=1]",
            "Threshold / [rating=2]",
            "Location / [region=1]",
            "Location / [region=2]",
            "Location / [plan=basic]",
        ],
        &[
            -1.443310948,
            0.2434037171,
            -0.4545275119,
            -0.009195346946,
            -1.594440739,
        ],
        &[
            0.4136757693,
            0.3829436692,
            0.4550253789,
            0.4758110728,
            0.3946490768,
        ],
    );
    // the -2 log likelihood includes the multinomial constant of the six subpopulations
    let fit = |row: &str| {
        number(cell(
            &tables,
            "Model Fitting Information",
            row,
            "-2 Log Likelihood",
        ))
    };
    assert_close(fit("Intercept Only"), 62.60801807, 1e-8, "-2LL 0");
    assert_close(fit("Final"), 42.65837727, 1e-8, "-2LL");
    let gof = |row: &str, column: &str| number(cell(&tables, "Goodness-of-Fit", row, column));
    assert_close(gof("Pearson", "Chi-Square"), 8.998051228, 1e-8, "Pearson");
    assert_close(gof("Deviance", "Chi-Square"), 10.27307606, 1e-8, "deviance");
    assert_eq!(gof("Deviance", "df"), 7.0);
    let parallel =
        |row: &str, column: &str| number(cell(&tables, "Test of Parallel Lines", row, column));
    assert_close(
        parallel("Null Hypothesis", "-2 Log Likelihood"),
        42.65837727,
        1e-8,
        "null",
    );
    assert_close(
        parallel("General", "-2 Log Likelihood"),
        39.23598455,
        1e-7,
        "general",
    );
    assert_close(
        parallel("General", "Chi-Square"),
        3.422392718,
        1e-6,
        "parallel chi-square",
    );
    assert_eq!(parallel("General", "df"), 3.0);
    assert!(
        engine
            .warnings()
            .iter()
            .any(|w| w.contains("1 of the 18 cells"))
    );
}

#[test]
fn default_print_and_optional_tables() {
    let mut engine = Engine::default();
    let titles = |tables: &[Table]| tables.iter().map(|t| t.title.clone()).collect::<Vec<_>>();
    let tables = run(
        &mut engine,
        &format!("{GROUPED}PLUM rating BY region plan."),
    );
    assert_eq!(
        titles(&tables),
        [
            "Case Processing Summary",
            "Model Fitting Information",
            "Goodness-of-Fit",
            "Pseudo R-Square",
            "Parameter Estimates"
        ]
    );
    let tables = run(
        &mut engine,
        "PLUM rating BY region plan /PRINT=COVB CORB HISTORY(2) CELLINFO /CRITERIA=CIN(90).",
    );
    assert_eq!(
        titles(&tables),
        [
            "Case Processing Summary",
            "Iteration History",
            "Model Fitting Information",
            "Parameter Covariance",
            "Parameter Correlation",
            "Cell Information",
        ]
    );
    let corr = tables
        .iter()
        .find(|t| t.title == "Parameter Correlation")
        .unwrap();
    assert_eq!(corr.rows.len(), 5);
    assert_eq!(corr.rows[2][3], Cell::Number(1.0));
    let info = tables
        .iter()
        .find(|t| t.title == "Cell Information")
        .unwrap();
    assert_eq!(info.rows.len(), 18);
}

#[test]
fn string_dependent_and_errors() {
    let mut engine = Engine::default();
    run(
        &mut engine,
        "DATA LIST FREE /grade (A1) x.
BEGIN DATA.
a 1 a 2 b 2 a 3 b 3 c 3 b 4 c 4 c 5 b 5 c 6 a 1
END DATA.
PLUM grade WITH x /SAVE=PREDCAT.",
    );
    let data = engine.dataset().unwrap();
    match data.variable("PRE_1").unwrap().values() {
        Values::String(v) => assert_eq!(v.get(0).trim_end(), "a"),
        Values::Numeric(_) => panic!("PREDCAT of a string dependent is a string"),
    }
    let error =
        run_error("DATA LIST FREE /y g (A1).\nBEGIN DATA.\n1 a 2 b\nEND DATA.\nPLUM y WITH g.");
    assert!(error.contains("numeric covariates"), "{error}");
    let error = run_error("DATA LIST FREE /y x.\nBEGIN DATA.\n1 1 1 2\nEND DATA.\nPLUM y WITH x.");
    assert!(error.contains("at least two values"), "{error}");
}

/// TEST: Wald tests of combinations of the thresholds and location parameters, each row alone
/// and the rows jointly. The expected values come from R: polr's estimates on the same cases,
/// their covariance from a numerical Hessian of the log-likelihood, and the tests computed
/// separately from the formula of the documentation.
#[test]
fn test_gives_wald_tests_of_combinations() {
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        &format!(
            "{DATA}PLUM delay BY region plan WITH dist /PRINT=PARAMETER\n\
             /TEST(0 0)='thresholds' INTERCEPT 1 -1 0; INTERCEPT 0 1 -1\n\
             /TEST(0.5)=region 1 -1 0\n\
             /TEST(0.05 2)=ALL 0 0 0 1 0 0 0 0 0; ALL 0 0 0 0 1 0 0 1 0\n\
             /TEST=region 0 0 1."
        ),
    );
    let nth = |title: &str, n: usize, row: &str, column: &str| {
        let found: Vec<&Table> = tables.iter().filter(|t| t.title == title).collect();
        number(cell(std::slice::from_ref(found[n]), title, row, column))
    };
    let ce = "Contrast Estimates";
    for (n, row, est, se, wald, p) in [
        (
            0,
            "L1",
            -1.52828964783,
            0.254894268696,
            35.9493631994,
            2.02512549463e-09,
        ),
        (
            0,
            "L2",
            -0.929978109818,
            0.234371994536,
            15.7446865968,
            7.24912621421e-05,
        ),
        (
            1,
            "L1",
            -0.547723236436,
            0.456647821697,
            5.26417563814,
            0.0217687302977,
        ),
        (
            2,
            "L1",
            0.0531691518583,
            0.0194178808626,
            0.0266368236705,
            0.870354734132,
        ),
        (
            2,
            "L2",
            1.87309556111,
            0.614435064255,
            0.0426581132423,
            0.836370367576,
        ),
    ] {
        let what = format!("test {n} {row}");
        assert_close(nth(ce, n, row, "Estimate"), est, 2e-6, &what);
        assert_close(nth(ce, n, row, "Std. Error"), se, 1e-4, &what);
        assert_close(nth(ce, n, row, "Wald"), wald, 1e-4, &what);
        assert_close(nth(ce, n, row, "Sig."), p, 1e-4, &what);
    }
    assert_close(
        nth(ce, 0, "L1", "95% Confidence Interval / Lower Bound"),
        -2.02787323434,
        1e-4,
        "lower bound",
    );
    let tr = "Test Results";
    assert_close(
        nth(tr, 0, "thresholds", "Wald"),
        56.0847879529,
        1e-4,
        "joint",
    );
    assert_eq!(nth(tr, 0, "thresholds", "df"), 2.0);
    assert_close(
        nth(tr, 0, "thresholds", "Sig."),
        6.62739776025e-13,
        1e-4,
        "joint p",
    );
    assert_close(
        nth(tr, 2, "Custom Hypothesis Test 3", "Wald"),
        0.0951365517442,
        1e-4,
        "joint",
    );
    assert_eq!(tables.iter().filter(|t| t.title == tr).count(), 3);
    assert!(
        engine
            .warnings()
            .iter()
            .any(|w| w.contains("Custom Hypothesis Test 4: L1 has no coefficient"))
    );
    assert!(
        run_error(&format!("{DATA}PLUM delay BY region /TEST=region 1 -1."))
            .contains("3 parameters")
    );
    assert!(
        run_error(&format!(
            "{DATA}PLUM delay BY region WITH dist /LOCATION=region /TEST=dist 1."
        ))
        .contains("not a LOCATION or SCALE effect")
    );
}
