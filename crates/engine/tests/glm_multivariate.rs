//! GLM with several dependent variables and no WSFACTOR (the multivariate model) on synthetic
//! data. The expected numbers come from a general statistics package used as a black box: its
//! multivariate linear model, sequential multivariate tests and model comparisons for the four
//! statistics with their F approximations, and matrix arithmetic on its fitted model for the
//! Type III SSCP matrices (sum-to-zero codings), Box's M, Bartlett's test and the custom tests.

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

fn value(tables: &[Table], title: &str, row: &str, column: &str) -> f64 {
    for table in tables.iter().filter(|t| t.title == title) {
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
            other => panic!("{title} / {row} / {column}: not a number: {other:?}"),
        };
    }
    panic!("no table {title} with row {row}");
}

fn close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() <= 1e-8 * expected.abs().max(1.0)
}

fn check(tables: &[Table], expected: &[(&str, &str, &str, f64)]) {
    let failures: Vec<String> = expected
        .iter()
        .filter_map(|&(title, row, column, want)| {
            let got = value(tables, title, row, column);
            (!close(got, want)).then(|| format!("{title} / {row} / {column}: {got} vs {want}"))
        })
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Three scores y1 to y3 of 30 made-up cases in the cells of the factors g (three levels) and
/// h (two), with a covariate x; unbalanced.
const DATA: &str = "DATA LIST FREE /g h x y1 y2 y3.
BEGIN DATA
1 1 8.4 20 37.9 13.9  1 2 6 17.9 36.6 15.1  1 1 6.6 16.9 29.5 12.3  1 2 5.7 18.2 31.5 13.1  1 1 5.5 16.3 27.7 9.2
1 2 4.3 13.2 30.1 9.7  1 1 3.3 15 25.5 9.4  1 2 5.5 14.1 27.7 10.1  1 1 3.1 13.1 25.8 10.7  2 1 5.4 16.5 37.3 11
2 1 8.5 19.9 29.9 14  2 1 8.4 16.6 32.6 11.3  2 1 4.2 15.1 28.3 11.4  2 1 8.4 18.9 30.6 11.3  2 2 4.1 20.4 30.4 13.7
2 2 8.5 20.1 35.1 10.7  2 2 7.8 18.8 32.1 14.5  2 2 7.1 23.4 35.1 12.4  2 2 2.3 14.9 25.2 11.5  2 2 4.3 18.9 32.8 9.3
3 1 8.7 19.8 31.7 12  3 1 8.7 20.2 34 14.3  3 1 7.7 22.9 32 12.1  3 1 3.6 20.4 28.4 12.3  3 1 8.1 18.5 27.4 11.8
3 1 4.7 18.2 28.1 10.4  3 2 4.1 18.2 27.1 13.1  3 2 7.2 20 31.6 10.4  3 2 4.3 20 27.2 11.8  3 2 3.6 17.2 30.8 13.2
END DATA.";

const MV: &str = "Multivariate Tests";
const BETWEEN: &str = "Tests of Between-Subjects Effects";

#[test]
fn type_three_multivariate_tests_match() {
    let mut engine = Engine::default();
    let t = run(
        &mut engine,
        &format!("{DATA}\nGLM y1 y2 y3 BY g h WITH x /PRINT=ETASQ OPOWER."),
    );
    let mut expected = Vec::new();
    // value, F, hypothesis df, error df, Sig., partial eta squared, noncentrality, power
    let rows: [(&str, &str, [f64; 8]); 12] = [
        (
            "x",
            "Pillai's Trace",
            [
                0.453967093341564,
                5.81974019265247,
                3.0,
                21.0,
                0.00466239436085517,
                0.453967093341564,
                17.4592205779574,
                0.905951765294760,
            ],
        ),
        (
            "x",
            "Roy's Largest Root",
            [
                0.831391456093207,
                5.81974019265245,
                3.0,
                21.0,
                0.00466239436085525,
                0.453967093341563,
                17.4592205779573,
                0.905951765294759,
            ],
        ),
        (
            "g",
            "Pillai's Trace",
            [
                0.497695102822232,
                2.42944298072880,
                6.0,
                44.0,
                0.0408534588183011,
                0.248847551411116,
                14.5766578843728,
                0.765381414159443,
            ],
        ),
        (
            "g",
            "Wilks' Lambda",
            [
                0.518766423680678,
                2.71878803529026,
                6.0,
                42.0,
                0.02536954546677303,
                0.279745584060272,
                16.3127282117416,
                0.815450657946213,
            ],
        ),
        (
            "g",
            "Hotelling's Trace",
            [
                0.895917755275729,
                2.98639251758576,
                6.0,
                40.0,
                0.01662757528009748,
                0.309372651776302,
                17.9183551055146,
                0.852853246123042,
            ],
        ),
        (
            "g",
            "Roy's Largest Root",
            [
                0.858976027592477,
                6.29915753567816,
                3.0,
                22.0,
                0.00300352224537735,
                0.462069448364495,
                18.8974726070345,
                0.930373888265408,
            ],
        ),
        (
            "h",
            "Wilks' Lambda",
            [
                0.889660565569643,
                0.868169356835497,
                3.0,
                21.0,
                0.473130457335135,
                0.110339434430357,
                2.60450807050649,
                0.206306885027589,
            ],
        ),
        (
            "g * h",
            "Pillai's Trace",
            [
                0.233587039954012,
                0.969745843737936,
                6.0,
                44.0,
                0.456815949355315,
                0.116793519977006,
                5.81847506242762,
                0.340912761125584,
            ],
        ),
        (
            "g * h",
            "Wilks' Lambda",
            [
                0.767612335620833,
                0.989637354737026,
                6.0,
                42.0,
                0.444642443312691,
                0.123865115623837,
                5.93782412842216,
                0.345687145898691,
            ],
        ),
        (
            "g * h",
            "Hotelling's Trace",
            [
                0.301178443956792,
                1.003928146522642,
                6.0,
                40.0,
                0.436334262214114,
                0.130880090915039,
                6.02356887913585,
                0.348248587391010,
            ],
        ),
        (
            "g * h",
            "Roy's Largest Root",
            [
                0.295897990250458,
                2.169918595170024,
                3.0,
                22.0,
                0.120342728677532,
                0.228334322976510,
                6.50975578551007,
                0.476993050605076,
            ],
        ),
        (
            "Intercept",
            "Wilks' Lambda",
            [
                0.0789734091457199,
                81.6374296832464,
                3.0,
                21.0,
                9.66898391301175e-12,
                0.921026590854280,
                244.912289049739,
                1.0,
            ],
        ),
    ];
    let columns = [
        "Value",
        "F",
        "Hypothesis df",
        "Error df",
        "Sig.",
        "Partial Eta Squared",
        "Noncent. Parameter",
        "Observed Power",
    ];
    let keys: Vec<String> = rows.iter().map(|(e, s, _)| format!("{e} / {s}")).collect();
    for ((_, _, values), key) in rows.iter().zip(&keys) {
        for (column, want) in columns.iter().zip(values) {
            expected.push((MV, key.as_str(), *column, *want));
        }
    }
    // the univariate tests of each dependent
    for (source, ss) in [
        ("Intercept", [526.560834478, 1600.61516823, 278.88461294]),
        ("x", [45.387336464, 111.276018785, 8.01993565697]),
        ("g", [42.4631487204, 12.8430334311, 1.08674813892]),
        ("h", [5.60259179512, 16.6283881377, 3.13135719508]),
        ("g * h", [15.3066097772, 1.34970791066, 0.156114708224]),
        ("Error", [73.9129968693, 203.060647882, 65.3508976764]),
    ] {
        for (dep, want) in ["y1", "y2", "y3"].iter().zip(ss) {
            let key: &'static str = Box::leak(format!("{source} / {dep}").into_boxed_str());
            expected.push((BETWEEN, key, "Type III Sum of Squares", want));
        }
    }
    let failures: Vec<String> = expected
        .iter()
        .filter_map(|&(title, row, column, want)| {
            let got = value(&t, title, row, column);
            let tolerance = if column == "Sig." || column == "Observed Power" {
                1e-7
            } else {
                1e-8
            };
            ((got - want).abs() > tolerance * want.abs().max(1.0))
                .then(|| format!("{title} / {row} / {column}: {got} vs {want}"))
        })
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    // each dependent's F in the univariate table equals UNIANOVA of that dependent alone
    let alone = run(&mut engine, "UNIANOVA y2 BY g h WITH x /PRINT=PARAMETER.");
    let glm = run(&mut engine, "GLM y1 y2 y3 BY g h WITH x /PRINT=PARAMETER.");
    for source in ["x", "g", "h", "g * h"] {
        let a = value(&alone, BETWEEN, source, "F");
        let b = value(&glm, BETWEEN, &format!("{source} / y2"), "F");
        assert!(close(a, b), "{source}: {a} vs {b}");
    }
    let a = value(&alone, "Parameter Estimates", "[g=1]", "B");
    let b = value(&glm, "Parameter Estimates", "y2 / [g=1]", "B");
    assert!(close(a, b), "{a} vs {b}");
}

#[test]
fn type_one_matches_the_sequential_tests() {
    let mut engine = Engine::default();
    let t = run(
        &mut engine,
        &format!("{DATA}\nGLM y1 y2 y3 BY g h WITH x /METHOD=SSTYPE(1)."),
    );
    check(
        &t,
        &[
            (MV, "Intercept / Pillai's Trace", "Value", 0.995149029906893),
            (MV, "Intercept / Pillai's Trace", "F", 1436.01033930251),
            (MV, "x / Wilks' Lambda", "Value", 0.522640039764806),
            (MV, "x / Wilks' Lambda", "F", 6.393539467718711),
            (MV, "g / Pillai's Trace", "F", 2.554554107410112),
            (MV, "g / Wilks' Lambda", "F", 2.824819455781187),
            (MV, "g / Hotelling's Trace", "Value", 0.922005200432876),
            (MV, "g / Hotelling's Trace", "F", 3.073350668109587),
            (MV, "g / Roy's Largest Root", "Value", 0.866697367237462),
            (MV, "g / Roy's Largest Root", "F", 6.355780693074721),
            (MV, "g / Roy's Largest Root", "Sig.", 2.87545689764184e-03),
            (MV, "h / Pillai's Trace", "Value", 0.118748766314845),
            (
                BETWEEN,
                "g / y1",
                "Type I Sum of Squares",
                44.13491996768865,
            ),
            (
                BETWEEN,
                "h / y2",
                "Type I Sum of Squares",
                16.87095839271489,
            ),
        ],
    );
}

#[test]
fn homogeneity_sscp_and_sphericity_match() {
    let mut engine = Engine::default();
    let t = run(
        &mut engine,
        &format!("{DATA}\nGLM y1 y2 y3 BY g h WITH x /PRINT=HOMOGENEITY RSSCP TEST(SSCP)."),
    );
    let boxm = "Box's Test of Equality of Covariance Matrices";
    let levene = "Levene's Test of Equality of Error Variances";
    let residual = "Residual SSCP Matrix";
    let bartlett = "Bartlett's Test of Sphericity";
    let sscp = "Between-Subjects SSCP Matrix";
    check(
        &t,
        &[
            (boxm, "Box's M", "Value", 50.7592611426),
            (boxm, "F", "Value", 1.08112260095),
            (boxm, "df1", "Value", 30.0),
            (boxm, "df2", "Value", 979.985928298),
            (boxm, "Sig.", "Value", 0.350889739457),
            (
                levene,
                "y1 / Based on Mean",
                "Levene Statistic",
                2.54520516889,
            ),
            (levene, "y2 / Based on Mean", "Sig.", 0.79799632151),
            (
                levene,
                "y3 / Based on Mean",
                "Levene Statistic",
                1.3275171454,
            ),
            (levene, "y3 / Based on Mean", "df2", 24.0),
            (
                residual,
                "Sum-of-Squares and Cross-Products / y1",
                "y2",
                48.9906412262284,
            ),
            (residual, "Covariance / y2", "y2", 8.82872382096293),
            (residual, "Correlation / y3", "y1", 0.351286036432360),
            (bartlett, "Likelihood Ratio", "Value", 1.55039104161e-05),
            (bartlett, "Approx. Chi-Square", "Value", 16.0373983254),
            (bartlett, "df", "Value", 5.0),
            (bartlett, "Sig.", "Value", 0.00679586964613),
            (sscp, "Hypothesis / g / y1", "y2", -15.606284490),
            (sscp, "Hypothesis / g * h / y3", "y3", 0.1561147082),
            (sscp, "Hypothesis / Intercept / y2", "y3", 668.1219512),
            (sscp, "Error / y3", "y1", 24.41445711),
        ],
    );
}

#[test]
fn custom_hypotheses_take_l_m_and_k() {
    let mut engine = Engine::default();
    // MMATRIX alone tests the intercept's L on the transformed variables
    let t = run(
        &mut engine,
        &format!(
            "{DATA}\nGLM y1 y2 y3 BY g h WITH x /PRINT=TEST(MMATRIX) \
             /MMATRIX='y1 - y2' y1 1 y2 -1; 'mean' ALL 1/3 1/3 1/3."
        ),
    );
    let k = "Contrast Results (K Matrix)";
    let mvr = "Multivariate Test Results";
    let uvr = "Univariate Test Results";
    check(
        &t,
        &[
            (
                "Transformation Coefficients (M Matrix)",
                "y2",
                "Transformed Variable / y1 - y2",
                -1.0,
            ),
            (k, "L1 / Contrast Estimate", "y1 - y2", -10.4351141525),
            (k, "L1 / Contrast Estimate", "mean", 16.2400430535),
            (k, "L1 / Std. Error", "y1 - y2", 1.70628576984),
            (k, "L1 / Std. Error", "mean", 0.994472274819),
            (mvr, "Pillai's Trace", "Value", 0.9208516638936668),
            (mvr, "Wilks' Lambda", "F", 127.979548290464),
            (mvr, "Hotelling's Trace", "Hypothesis df", 2.0),
            (mvr, "Roy's Largest Root", "Error df", 22.0),
            (uvr, "Contrast / y1 - y2", "Sum of Squares", 291.07005951),
            (uvr, "Contrast / mean", "Sum of Squares", 704.981107387),
            (uvr, "Error / y1 - y2", "Sum of Squares", 178.992362299),
            (uvr, "Error / mean", "Sum of Squares", 60.8017839336),
        ],
    );
    // LMATRIX with a K matrix of one row and three columns, and a contrast of g
    let t = run(
        &mut engine,
        "GLM y1 y2 y3 BY g h WITH x /PRINT=ETASQ OPOWER \
         /LMATRIX='g1 vs g3' g 1 0 -1 g*h 1/2 1/2 0 0 -1/2 -1/2 /KMATRIX=1 0 0 \
         /CONTRAST(g)=SIMPLE(1).",
    );
    check(
        &t,
        &[
            (k, "L1 / Contrast Estimate", "y1", -3.04751020673),
            (
                k,
                "L1 / Difference (Estimate - Hypothesized)",
                "y1",
                -4.04751020673,
            ),
            (k, "L1 / Contrast Estimate", "y3", -0.451939217422),
            (k, "L1 / Std. Error", "y2", 1.3903669043),
            (
                k,
                "Level 2 vs. Level 1 / Contrast Estimate",
                "y1",
                1.71338391689,
            ),
            (
                k,
                "Level 2 vs. Level 1 / Contrast Estimate",
                "y3",
                0.0893262344573,
            ),
        ],
    );
    let of = |prefix: &str| -> Vec<Table> {
        t.iter()
            .filter(|t| t.caption.as_deref().unwrap_or("").starts_with(prefix))
            .cloned()
            .collect()
    };
    check(
        &of("g1 vs g3"),
        &[
            (uvr, "Contrast / y1", "Sum of Squares", 74.8195331055),
            (uvr, "Contrast / y3", "Sum of Squares", 0.932822788207),
            (mvr, "Pillai's Trace", "Value", 0.589604172615442),
            (mvr, "Wilks' Lambda", "F", 10.0567036331992),
            (mvr, "Roy's Largest Root", "Sig.", 0.000259784633337007),
            (
                mvr,
                "Hotelling's Trace",
                "Noncent. Parameter",
                30.1701108995977,
            ),
            (
                mvr,
                "Hotelling's Trace",
                "Observed Power",
                0.993000472174744,
            ),
        ],
    );
    // the simple contrasts of g span the effect of g: its Type III multivariate test
    check(
        &of("g Simple"),
        &[
            (mvr, "Pillai's Trace", "Value", 0.497695102822232),
            (mvr, "Wilks' Lambda", "F", 2.71878803529026),
            (mvr, "Roy's Largest Root", "Error df", 22.0),
        ],
    );
}

#[test]
fn means_post_hoc_and_saved_values_come_per_dependent() {
    let mut engine = Engine::default();
    let t = run(
        &mut engine,
        &format!(
            "{DATA}\nGLM y1 y2 y3 BY g WITH x /EMMEANS=TABLES(g) /POSTHOC=g(LSD) /SAVE=PRED RESID."
        ),
    );
    let alone = run(&mut engine, "UNIANOVA y3 BY g WITH x /EMMEANS=TABLES(g).");
    let estimates: Vec<&Table> = t
        .iter()
        .filter(|t| t.title == "Estimated Marginal Means: g")
        .collect();
    assert_eq!(estimates.len(), 3);
    let title = "Estimated Marginal Means: g";
    let a = value(&alone, title, "2", "Mean");
    let b = value(std::slice::from_ref(estimates[2]), title, "2", "Mean");
    assert!(close(a, b), "{a} vs {b}");
    assert!(t.iter().any(|t| t.title == "Multiple Comparisons: y2"));
    let data = engine.dataset().unwrap();
    for name in ["PRE_1", "PRE_2", "PRE_3", "RES_1", "RES_2", "RES_3"] {
        assert!(data.variable(name).is_ok(), "{name}");
    }
}

#[test]
fn errors_name_the_problem() {
    assert_eq!(
        run_error(&format!("{DATA}\nGLM y1 y2 BY g /MMATRIX=y3 1.")),
        "MMATRIX names y3, which is not a dependent variable"
    );
    assert_eq!(
        run_error(&format!("{DATA}\nGLM y1 y2 BY g /MMATRIX=ALL 1 2 3.")),
        "ALL in MMATRIX needs 2 values, one per dependent variable, but has 3"
    );
    assert_eq!(
        run_error(&format!(
            "{DATA}\nGLM y1 y2 BY g /LMATRIX=g 1 -1 0 /KMATRIX=0."
        )),
        "KMATRIX needs a row for each of the 1 rows of the L matrix, each with a value for each of the 2 columns of the M matrix"
    );
    assert_eq!(
        run_error(&format!(
            "{DATA}\nGLM y1 y2 y3 /WSFACTOR=t 3 /MMATRIX=ALL 1 -1 0."
        )),
        "MMATRIX with WSFACTOR is not supported yet"
    );
    assert_eq!(
        run_error(&format!(
            "{DATA}\nGLM y1 y2 BY g /INTERCEPT=EXCLUDE /MMATRIX=y1 1 y2 -1."
        )),
        "KMATRIX or MMATRIX without LMATRIX tests the intercept, which the model leaves out or makes redundant"
    );
    assert_eq!(
        run_error(&format!("{DATA}\nGLM y1 y2 BY g /MMATRIX=y1 1 y2 -1 y1 2.")),
        "MMATRIX names y1 twice in one column"
    );
    assert_eq!(
        run_error(&format!("{DATA}\nGLM y1 y2 BY g /MMATRIX=ALL 1 1 y1 2.")),
        "ALL in MMATRIX gives a whole column; end the column with ';' before naming dependents"
    );
    assert_eq!(
        run_error(&format!(
            "{DATA}\nGLM y1 y2 BY g /MMATRIX='z' y1 0; y1 1 y2 -1."
        )),
        "MMATRIX column z has no coefficient other than 0"
    );
    assert_eq!(
        run_error(&format!("{DATA}\nUNIANOVA y1 y2 BY g.")),
        "UNIANOVA takes one dependent variable; GLM analyzes several"
    );
    let mut engine = Engine::default();
    run(&mut engine, &format!("{DATA}\nGLM y1 BY g /PRINT=RSSCP."));
    let want = "PRINT RSSCP applies only to GLM with several dependent variables and is ignored";
    assert!(
        engine.warnings().iter().any(|w| w.ends_with(want)),
        "{:?}",
        engine.warnings()
    );
}

/// KMATRIX alone tests the intercept's Type III L matrix: with K = 0 its F is the F of the
/// intercept in the Tests of Between-Subjects Effects, also with one dependent.
#[test]
fn kmatrix_alone_tests_the_intercept() {
    let mut engine = Engine::default();
    let t = run(
        &mut engine,
        &format!("{DATA}\nUNIANOVA y2 BY g h WITH x /KMATRIX=0."),
    );
    let custom = value(&t, "Test Results", "Contrast", "F");
    let between = value(&t, BETWEEN, "Intercept", "F");
    assert!(close(custom, between), "{custom} vs {between}");
    let t = run(&mut engine, "UNIANOVA y2 BY g h WITH x /KMATRIX=5.");
    let estimate = value(
        &t,
        "Contrast Results (K Matrix)",
        "L1 / Contrast Estimate",
        "y2",
    );
    let diff = value(
        &t,
        "Contrast Results (K Matrix)",
        "L1 / Difference (Estimate - Hypothesized)",
        "y2",
    );
    assert!(close(diff, estimate - 5.0), "{diff} vs {estimate}");
}

/// The tests do not depend on the units of the dependents: one in millions and one in
/// millionths give the same Box's M and multivariate tests. Bartlett's test compares with the
/// identity, so it changes, but it is still given.
#[test]
fn units_of_the_dependents_do_not_matter() {
    let mut engine = Engine::default();
    let syntax = "/PRINT=HOMOGENEITY RSSCP.";
    let plain = run(
        &mut engine,
        &format!("{DATA}\nGLM y1 y2 y3 BY g h WITH x {syntax}"),
    );
    let scaled = run(
        &mut engine,
        &format!(
            "COMPUTE big = y1 * 1e6.\nCOMPUTE small = y2 / 1e6.\nGLM big small y3 BY g h WITH x {syntax}"
        ),
    );
    for (title, row, column) in [
        (
            "Box's Test of Equality of Covariance Matrices",
            "Box's M",
            "Value",
        ),
        (MV, "g / Wilks' Lambda", "F"),
        (MV, "g / Roy's Largest Root", "Hypothesis df"),
    ] {
        let a = value(&plain, title, row, column);
        let b = value(&scaled, title, row, column);
        assert!(
            (a - b).abs() <= 1e-6 * a.abs().max(1.0),
            "{title} / {row}: {a} vs {b}"
        );
    }
    let chi = value(
        &scaled,
        "Bartlett's Test of Sphericity",
        "Approx. Chi-Square",
        "Value",
    );
    assert!(chi.is_finite() && chi > 0.0, "{chi}");
}

/// A dependent that the model fits exactly, or that is constant, adds no error variance: its
/// rounding noise does not count, so the tests rest on the other dependents.
#[test]
fn an_exactly_fitted_dependent_adds_nothing() {
    let mut engine = Engine::default();
    let two = run(
        &mut engine,
        &format!("{DATA}\nGLM y1 y2 BY g WITH x /PRINT=RSSCP HOMOGENEITY."),
    );
    let three = run(
        &mut engine,
        "COMPUTE z = x * 1.7.\nCOMPUTE c = 0.1.\nGLM y1 y2 z BY g WITH x /PRINT=RSSCP.",
    );
    for row in ["x / Pillai's Trace", "g / Wilks' Lambda"] {
        let a = value(&two, MV, row, "F");
        let b = value(&three, MV, row, "F");
        assert!(close(a, b), "{row}: {a} vs {b}");
        assert_eq!(
            value(&three, MV, row, "Hypothesis df"),
            value(&two, MV, row, "Hypothesis df")
        );
    }
    // the residual SSCP is singular: no chi-square
    let t = three
        .iter()
        .find(|t| t.title == "Bartlett's Test of Sphericity")
        .unwrap();
    assert_eq!(t.rows[1][1], Cell::Empty);
    let with_constant = run(&mut engine, "GLM y1 y2 c BY g /PRINT=HOMOGENEITY.");
    assert!(
        !with_constant
            .iter()
            .any(|t| t.title == "Box's Test of Equality of Covariance Matrices"),
        "a constant dependent leaves every cell singular"
    );
}
