//! CATREG on synthetic data. The expected numbers come from R run as a black box: least
//! squares on the standardized variables (all numeric), least squares on factor dummies
//! (numeric response, nominal predictors: the additive model is the optimum), the first
//! canonical correlation of the dummies (everything nominal), the ridge solution
//! (R + kI)b = r and a coordinate descent lasso, and a separate script that runs the manual's
//! alternating least squares steps (ordinal and nominal levels, case weights) with the same
//! start and stopping rule.

use statscraft_data::Values;
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

/// Thirty invented diary days: mood rated 1 to 5, hours of sleep in 4 bands, the kind of
/// diet (1 to 3), the walking time in 6 bands, and a case weight.
const DATA: &str = "DATA LIST FREE / mood sleep diet walk wt.
BEGIN DATA
2 1 1 3 2  1 3 2 1 1  3 1 1 5 1  5 2 1 6 1  5 4 3 3 2  4 3 1 2 1  1 3 2 3 2  2 3 1 3 1
5 2 1 4 1  3 1 2 4 1  5 2 1 4 1  2 2 3 2 3  4 4 1 5 1  2 3 2 4 1  3 3 2 6 1  4 4 1 1 1
1 1 2 4 1  4 3 3 4 2  2 2 1 1 2  2 4 3 3 1  1 4 2 3 1  3 2 3 3 1  1 1 3 2 3  3 3 2 6 3
4 4 3 2 1  1 1 1 3 3  3 1 3 2 2  3 1 2 6 1  2 1 1 5 2  2 2 2 6 1
END DATA.
";

const NUMERIC: &str = "CATREG VARIABLES=mood sleep diet walk /ANALYSIS=mood(LEVEL=NUME) WITH sleep diet walk(LEVEL=NUME)";

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

fn value(tables: &[Table], title: &str, row: &str, column: &str) -> f64 {
    let t = tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"));
    let r = t
        .rows
        .iter()
        .position(|r| r[0] == Cell::Text(row.into()))
        .unwrap_or_else(|| panic!("{title}: no row {row}"));
    let c = t
        .column_titles()
        .iter()
        .position(|c| c == column)
        .unwrap_or_else(|| panic!("{title}: no column {column} in {:?}", t.column_titles()));
    match &t.rows[r][c] {
        Cell::Number(x) => *x,
        other => panic!("{title} / {row} / {column}: {other:?}"),
    }
}

fn check(tables: &[Table], title: &str, row: &str, column: &str, expected: f64, tolerance: f64) {
    let actual = value(tables, title, row, column);
    assert!(
        (actual - expected).abs() <= tolerance,
        "{title} / {row} / {column}: {actual} against {expected}"
    );
}

fn r2(t: &[Table]) -> f64 {
    value(t, "Model Summary", "Standardized Data", "R Square")
}

fn betas(t: &[Table], expected: [f64; 3], tolerance: f64) {
    for (name, b) in ["sleep", "diet", "walk"].iter().zip(expected) {
        check(t, "Coefficients", name, "Beta", b, tolerance);
    }
}

fn quantifications(t: &[Table], var: &str, expected: &[f64], tolerance: f64) {
    let table = t
        .iter()
        .find(|t| t.title == format!("Quantifications: {var}"))
        .expect("a quantification table");
    for (row, e) in table.rows.iter().zip(expected) {
        match row[2] {
            Cell::Number(x) => assert!((x - e).abs() <= tolerance, "{var}: {x} against {e}"),
            ref other => panic!("{other:?}"),
        }
    }
}

fn numbers(engine: &Engine, name: &str) -> Vec<Option<f64>> {
    match engine.dataset().unwrap().variable(name).unwrap().values() {
        Values::Numeric(v) => v.iter().collect(),
        _ => panic!("{name} is not numeric"),
    }
}

#[test]
fn numeric_levels_give_least_squares_on_standardized_variables() {
    let t = tables(&format!(
        "WEIGHT BY wt.\n{NUMERIC} /PRINT=R COEFF ANOVA CORR OCORR QUANT(walk) HISTORY."
    ));
    // weighted least squares of the standardized response on the standardized predictors
    let b = [0.3990850046691, -0.0104568751965, 0.2753100763573];
    betas(&t, b, 1e-8);
    let r2_lm = 0.23488862389;
    assert!((r2(&t) - r2_lm).abs() < 1e-9);
    check(
        &t,
        "Model Summary",
        "Standardized Data",
        "Multiple R",
        r2_lm.sqrt(),
        1e-9,
    );
    check(
        &t,
        "Model Summary",
        "Standardized Data",
        "Apparent Prediction Error",
        1.0 - r2_lm,
        1e-9,
    );
    // 45 weighted cases, 3 predictors of 1 degree of freedom each
    let n = 45.0;
    let adjusted = 1.0 - (1.0 - r2_lm) * (n - 1.0) / (n - 4.0);
    check(
        &t,
        "Model Summary",
        "Standardized Data",
        "Adjusted R Square",
        adjusted,
        1e-9,
    );
    check(
        &t,
        "Model Summary",
        "Standardized Data",
        "Adjusted R Square (Optimal Scaling df)",
        adjusted,
        1e-9,
    );
    check(&t, "ANOVA", "Regression", "Sum of Squares", n * r2_lm, 1e-7);
    check(&t, "ANOVA", "Residual", "df", n - 4.0, 1e-12);
    let f = (r2_lm / 3.0) / ((1.0 - r2_lm) / (n - 4.0));
    check(&t, "ANOVA", "Regression", "F", f, 1e-6);
    // correlations, tolerances (1 over the diagonal of the inverse correlations), Pratt
    let zero = [0.39720585302277, 0.00318015494892, 0.27751609994107];
    let tol = [0.966201942101, 0.923200367489, 0.954008989596];
    for (k, name) in ["sleep", "diet", "walk"].iter().enumerate() {
        let c = "Correlations and Tolerance";
        check(&t, c, name, "Zero-Order", zero[k], 1e-9);
        check(&t, c, name, "Tolerance After Transformation", tol[k], 1e-9);
        check(&t, c, name, "Tolerance Before Transformation", tol[k], 1e-9);
        check(&t, c, name, "Part", b[k] * tol[k].sqrt(), 1e-8);
        let partial = b[k] / ((1.0 - r2_lm) / tol[k] + b[k] * b[k]).sqrt();
        check(&t, c, name, "Partial", partial, 1e-8);
        check(&t, c, name, "Importance", b[k] * zero[k] / r2_lm, 1e-8);
        check(&t, "Coefficients", name, "df", 1.0, 0.0);
    }
    // numeric quantifications of walk: the weighted standardized codes
    quantifications(
        &t,
        "walk",
        &[
            -1.572667301990,
            -0.917389259494,
            -0.262111216998,
            0.393166825497,
            1.048444867993,
            1.703722910489,
        ],
        1e-9,
    );
    let o = value(&t, "Correlations Original Variables", "sleep", "walk");
    check(
        &t,
        "Correlations Transformed Variables",
        "sleep",
        "walk",
        o,
        1e-12,
    );
    // a bootstrap standard error near the least squares one
    let se = value(&t, "Coefficients", "sleep", "Std. Error (Bootstrap, 1000)");
    assert!(se > 0.05 && se < 0.4, "{se}");
}

#[test]
fn nominal_predictors_give_the_additive_factor_model() {
    let t = tables(
        "CATREG VARIABLES=mood sleep diet walk /ANALYSIS=mood(LEVEL=NUME) WITH sleep diet walk(LEVEL=NOMI)
         /CRITITER=.000001 /MAXITER=1000 /PRINT=R COEFF ANOVA.",
    );
    // R square of least squares on dummies of the three factors
    assert!((r2(&t) - 0.699474546194).abs() < 2e-6, "{}", r2(&t));
    // nominal degrees of freedom: categories less 1
    check(&t, "Coefficients", "sleep", "df", 3.0, 0.0);
    check(&t, "Coefficients", "walk", "df", 5.0, 0.0);
    check(
        &t,
        "ANOVA (Optimal Scaling df)",
        "Regression",
        "df",
        10.0,
        0.0,
    );
}

#[test]
fn all_nominal_gives_the_first_canonical_correlation() {
    let t = tables(
        "CATREG VARIABLES=mood sleep diet walk /ANALYSIS=mood(LEVEL=NOMI) WITH sleep diet walk(LEVEL=NOMI)
         /CRITITER=.000001 /MAXITER=1000 /PRINT=R.",
    );
    // the squared first canonical correlation of the response dummies and the predictor dummies
    assert!((r2(&t) - 0.85866109047).abs() < 2e-6, "{}", r2(&t));
}

#[test]
fn ordinal_and_nominal_levels_follow_the_alternating_steps() {
    let t = tables(
        "CATREG VARIABLES=mood sleep diet walk /ANALYSIS=mood(LEVEL=NUME) WITH sleep(LEVEL=ORDI) diet(LEVEL=NOMI) walk(LEVEL=NUME)
         /CRITITER=.000001 /PRINT=R COEFF QUANT(sleep diet).",
    );
    // the separate script, same start (numeric first phase) and stopping rule
    assert!((r2(&t) - 0.481233666997).abs() < 1e-9);
    betas(&t, [0.395446812122, -0.563188317503, 0.482861356707], 1e-8);
    quantifications(
        &t,
        "sleep",
        &[
            -1.464945793569,
            0.413681292728,
            0.413681292728,
            1.163215458534,
        ],
        1e-8,
    );
    quantifications(
        &t,
        "diet",
        &[-0.735893458940, 1.413615233206, -0.663178853096],
        1e-8,
    );
    // the converged optimum of the same script
    assert!((r2(&t) - 0.48123367173).abs() < 1e-6);
    // ordinal degrees of freedom: distinct values less 1 (two codes tie)
    check(&t, "Coefficients", "sleep", "df", 2.0, 0.0);
}

#[test]
fn weighted_ordinal_response_follows_the_alternating_steps() {
    let t = tables(
        "WEIGHT BY wt.
         CATREG VARIABLES=mood sleep diet walk /ANALYSIS=mood(LEVEL=ORDI) WITH sleep(LEVEL=ORDI) diet(LEVEL=NOMI) walk(LEVEL=ORDI)
         /CRITITER=.000001 /PRINT=R COEFF QUANT(mood walk).",
    );
    assert!((r2(&t) - 0.742329709673).abs() < 1e-9);
    betas(&t, [0.624673007753, -0.697827820227, 0.599819608442], 1e-8);
    quantifications(
        &t,
        "mood",
        &[
            -1.245343342850,
            -0.158686349443,
            -0.158686349443,
            1.577142073502,
            1.577142073502,
        ],
        1e-8,
    );
    quantifications(
        &t,
        "walk",
        &[
            -0.811701783511,
            -0.811701783511,
            -0.811701783511,
            1.081060705998,
            1.081060705998,
            1.432040055547,
        ],
        1e-8,
    );
}

#[test]
fn whole_weights_equal_repeated_cases_and_linear_splines_equal_numeric() {
    let spec = "/ANALYSIS=mood(LEVEL=NUME) WITH sleep(LEVEL=ORDI) diet(LEVEL=NOMI) walk(LEVEL=SPORD DEGREE=1 INKNOT=0)
                /CRITITER=.000001 /PRINT=R COEFF";
    let weighted = tables(&format!(
        "WEIGHT BY wt.\nCATREG VARIABLES=mood sleep diet walk {spec}."
    ));
    // every case written out as often as its weight
    let body = DATA
        .split("BEGIN DATA")
        .nth(1)
        .unwrap()
        .split("END DATA")
        .next()
        .unwrap();
    let numbers: Vec<&str> = body.split_whitespace().collect();
    let mut rows = String::new();
    for case in numbers.chunks(5) {
        for _ in 0..case[4].parse::<usize>().unwrap() {
            rows.push_str(&format!(
                "{} {} {} {} 1\n",
                case[0], case[1], case[2], case[3]
            ));
        }
    }
    let repeated = run(
        &mut Engine::default(),
        &format!(
            "DATA LIST FREE / mood sleep diet walk wt.\nBEGIN DATA\n{rows}END DATA.\nCATREG VARIABLES=mood sleep diet walk {spec}."
        ),
    );
    assert!((r2(&weighted) - r2(&repeated)).abs() < 1e-10);
    for name in ["sleep", "diet", "walk"] {
        let b = value(&repeated, "Coefficients", name, "Beta");
        check(&weighted, "Coefficients", name, "Beta", b, 1e-10);
    }
    // a rising line of degree 1 without interior knots is the numeric level
    let numeric = tables(&format!(
        "WEIGHT BY wt.\nCATREG VARIABLES=mood sleep diet walk {}.",
        spec.replace("LEVEL=SPORD DEGREE=1 INKNOT=0", "LEVEL=NUME")
    ));
    assert!((r2(&weighted) - r2(&numeric)).abs() < 1e-10);
    for name in ["sleep", "diet", "walk"] {
        let b = value(&numeric, "Coefficients", name, "Beta");
        check(&weighted, "Coefficients", name, "Beta", b, 1e-10);
    }
}

#[test]
fn ridge_and_lasso_solve_their_penalized_problems() {
    // ridge: (R + kI) b = r on the unweighted standardized data
    let t = tables(&format!(
        "{NUMERIC} /REGULARIZATION=RIDGE(.5 .5 0) /PRINT=REGU COEFF."
    ));
    betas(
        &t,
        [0.1921450350406, -0.0839542499369, 0.1653954087649],
        1e-8,
    );
    // lasso 0.2: the coordinate descent solution, whose gradient is 0.1 in size everywhere
    let t = tables(&format!(
        "{NUMERIC} /REGULARIZATION=LASSO(.2 .2 0) /CRITITER=.000001 /PRINT=REGU COEFF."
    ));
    betas(&t, [0.176601368496, -0.025082588941, 0.155947470109], 1e-5);
    check(&t, "Regularized Models", "1", "Lasso Penalty", 0.2, 0.0);
    // a grid without resampling shows the model table only
    let mut engine = Engine::default();
    let t = run(
        &mut engine,
        &format!(
            "{DATA}DATASET DECLARE regu.\n{NUMERIC} /REGULARIZATION=ENET(0 .5 .5)(0 .4 .2)(regu) /PRINT=REGU COEFF /PLOT=REGU(.5)."
        ),
    );
    let titles: Vec<&str> = t.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(titles, ["Case Processing Summary", "Regularized Models"]);
    // ridge 0 and lasso 0 is least squares
    check(&t, "Regularized Models", "1", "sleep", 0.314407030037, 1e-6);
    let charts = engine.take_charts();
    assert_eq!(charts.len(), 1);
    assert_eq!(charts[0].title, "Elastic Net Paths, Ridge Penalty 0.5");
    run(&mut engine, "DATASET ACTIVATE regu.");
    assert_eq!(numbers(&engine, "LASSO").len(), 6);
}

#[test]
fn supplementary_cases_are_predicted_from_the_active_fit() {
    let mut engine = Engine::default();
    let t = run(
        &mut engine,
        &format!("{DATA}{NUMERIC} /SUPPLEMENTARY=OBJECT(1 TO 5) /PRINT=R /SAVE=TRDATA PRED RES."),
    );
    // least squares on cases 6 to 30, standardized by their own mean and deviation
    let pred = [
        -0.1588462265286,
        -0.0546681390286,
        -0.0181788062472,
        0.2862121082195,
        0.0967875847082,
    ];
    let res = [
        -0.396816156363,
        -1.318144806939,
        0.279666986431,
        1.609577198116,
        1.799001721628,
    ];
    let (p, r) = (numbers(&engine, "PRE_1"), numbers(&engine, "RES_1"));
    for k in 0..5 {
        assert!((p[k].unwrap() - pred[k]).abs() < 1e-8, "{k}: {:?}", p[k]);
        assert!((r[k].unwrap() - res[k]).abs() < 1e-8);
    }
    // the residual is the quantified response less the prediction
    let y = numbers(&engine, "TRA1_1");
    assert!((y[9].unwrap() - p[9].unwrap() - r[9].unwrap()).abs() < 1e-12);
    assert!(numbers(&engine, "TRA4_1")[0].is_some());
    // the test error is the mean of the squared residuals of the supplementary cases
    let test = res.iter().map(|e| e * e).sum::<f64>() / 5.0;
    check(
        &t,
        "Prediction Error",
        "Supplementary cases (test)",
        "Prediction Error",
        test,
        1e-8,
    );
}

#[test]
fn resampling_and_systematic_starts() {
    let mut engine = Engine::default();
    let ordinal = "CATREG VARIABLES=mood sleep diet walk /ANALYSIS=mood(LEVEL=NUME) WITH sleep(LEVEL=ORDI) diet(LEVEL=NOMI) walk(LEVEL=ORDI)";
    let t = run(
        &mut engine,
        &format!(
            "{DATA}SET SEED=7.\nDATASET DECLARE signs.\n{ordinal} /INITIAL=MULTISTART(ALL)(signs) /RESAMPLE=CROSSVAL(5) /PRINT=R."
        ),
    );
    let best = r2(&t);
    let cv = value(
        &t,
        "Prediction Error",
        "Cross validation, 5 folds",
        "Prediction Error",
    );
    assert!(cv > 1.0 - best && cv < 2.0, "{cv}");
    // every sign pattern of the two ordinal predictors, the best one reported
    run(&mut engine, "DATASET ACTIVATE signs.");
    let fits = numbers(&engine, "RSQ");
    assert_eq!(fits.len(), 4);
    let top = fits.iter().map(|f| f.unwrap()).fold(f64::MIN, f64::max);
    assert!((top - best).abs() < 1e-6, "{top} {best}");
    assert!(best >= 0.48);
    // the same seed repeats the cross validation
    let again = tables(&format!(
        "SET SEED=7.\n{ordinal} /RESAMPLE=CROSSVAL(5) /PRINT=R."
    ));
    let again2 = tables(&format!(
        "SET SEED=7.\n{ordinal} /RESAMPLE=CROSSVAL(5) /PRINT=R."
    ));
    let e = |t: &[Table]| {
        value(
            t,
            "Prediction Error",
            "Cross validation, 5 folds",
            "Prediction Error",
        )
    };
    assert_eq!(e(&again), e(&again2));
    let boot = tables(&format!(
        "SET SEED=7.\nDATASET DECLARE s2.\n{ordinal} /INITIAL=MULTISTART(10)(s2) /RESAMPLE=BOOTSTRAP(20) /PRINT=R."
    ));
    let b = value(
        &boot,
        "Prediction Error",
        ".632 bootstrap, 20 samples",
        "Prediction Error",
    );
    assert!(b.is_finite() && b > 0.0);
}

#[test]
fn charts_files_and_errors() {
    let mut engine = Engine::default();
    run(
        &mut engine,
        &format!(
            "{DATA}DATASET DECLARE codes.\nCATREG VARIABLES=mood sleep diet walk /ANALYSIS=mood WITH sleep diet(LEVEL=NOMI) walk
             /PRINT=NONE /PLOT=TRANS(mood TO diet)(0) RESID(walk) /OUTFILE=DISCRDATA(codes)."
        ),
    );
    let charts: Vec<String> = engine.take_charts().into_iter().map(|c| c.title).collect();
    assert_eq!(
        charts,
        [
            "Transformation: mood",
            "Transformation: sleep",
            "Transformation: diet",
            "Residuals: walk"
        ]
    );
    run(&mut engine, "DATASET ACTIVATE codes.");
    assert_eq!(numbers(&engine, "walk")[0], Some(3.0));
    assert!(
        run_error("CATREG VARIABLES=mood sleep /ANALYSIS=mood WITH sleep /PLOT=RESID(mood).")
            .contains("predictors only")
    );
    assert!(
        run_error(
            "CATREG VARIABLES=mood sleep /ANALYSIS=mood WITH sleep(LEVEL=NOMI) /INITIAL=FIXSIGNS(1)('x')."
        )
        .contains("ORDI")
    );
    assert!(run_error("CATREG VARIABLES=mood sleep /ANALYSIS=mood WITH mood.").contains("twice"));
    assert!(
        run_error("CATREG VARIABLES=mood sleep /ANALYSIS=mood WITH sleep /RESAMPLE=CROSSVAL(40).")
            .contains("folds")
    );
}

// ---- spline levels against the reference package ----

/// Seventy-two invented cases: a response that rises and falls with x (12 bands) and rises with z.
const SPLINE_HUMP: &str = "DATA LIST FREE /y x z.
BEGIN DATA
3 8 2  7 1 1  2 10 5  2 12 3  9 3 5  6 12 5
2 10 4  7 12 6  2 9 3  5 9 6  10 4 3  8 4 3
5 10 5  9 7 6  8 6 5  8 5 2  3 12 4  9 7 6
8 8 7  4 10 5  11 2 6  6 7 6  10 5 6  8 6 2
7 8 6  9 2 4  3 9 4  3 12 1  8 1 6  7 7 6
8 2 3  6 10 7  11 4 6  9 1 6  7 1 4  3 11 5
3 11 4  2 12 2  5 8 2  8 2 1  9 2 2  9 1 4
5 11 3  5 9 6  3 8 2  12 5 7  10 5 5  2 9 1
4 9 7  8 12 7  10 5 3  8 7 2  5 5 1  8 6 6
10 3 5  4 10 4  10 3 2  9 2 5  7 11 7  3 7 1
9 1 6  1 9 1  10 3 4  8 6 4  8 6 4  3 9 2
8 5 1  4 7 3  9 3 6  8 6 6  5 12 6  9 4 5
END DATA.
";

/// Seventy-two invented cases: a response that rises with x (12 bands) and with z.
const SPLINE_RISING: &str = "DATA LIST FREE /y x z.
BEGIN DATA
5 2 4  7 4 3  9 11 7  5 1 4  8 10 3  10 11 6
10 12 5  9 10 2  9 7 7  8 4 7  5 4 1  8 3 7
8 7 1  4 1 2  7 9 1  4 5 1  8 8 1  7 9 1
6 4 4  9 11 1  9 11 4  10 12 5  9 9 4  6 2 5
9 12 6  6 3 5  9 10 1  5 5 2  5 1 7  3 2 1
13 12 7  12 8 6  8 5 7  6 8 1  8 9 4  5 3 4
8 6 5  8 11 4  4 2 3  10 9 7  9 7 6  7 4 3
4 5 2  6 3 2  12 10 7  6 5 4  7 12 3  7 2 6
5 1 2  7 2 3  9 10 3  10 12 3  11 9 4  8 9 1
9 9 5  12 11 7  1 2 1  3 1 3  9 12 6  8 9 2
9 3 7  10 5 7  11 12 4  7 5 3  4 2 2  8 11 3
7 5 2  10 10 6  8 7 5  5 2 1  6 4 6  8 6 5
END DATA.
";

/// The R square of the response on a predictor with a spline level and a numeric covariate.
fn spline_r2(data: &str, level: &str) -> f64 {
    let mut engine = Engine::default();
    let t = run(
        &mut engine,
        &format!(
            "{data}CATREG VARIABLES=y x z /ANALYSIS=y(LEVEL=NUME) WITH x(LEVEL={level}) z(LEVEL=NUME) \
             /INITIAL=NUMERICAL /PRINT=R."
        ),
    );
    r2(&t)
}

#[test]
fn spline_nominal_levels_equal_the_least_squares_fit_on_a_spline_basis() {
    // R: lm(y ~ splines::bs(x, degree, knots = quantile(x, k / (inknot + 1), type = 7),
    //        intercept = TRUE) + z); the interior knots sit at quantiles of the cases' codes.
    let cases = [
        ("SPNOM DEGREE=1 INKNOT=2", 0.830759),
        ("SPNOM DEGREE=2 INKNOT=1", 0.833721),
        ("SPNOM DEGREE=2 INKNOT=3", 0.833876),
        ("SPNOM DEGREE=3 INKNOT=2", 0.835156),
        ("NOMI", 0.844672), // lm(y ~ factor(x) + z)
        ("NUME", 0.659043), // lm(y ~ x + z)
    ];
    for (level, expected) in cases {
        let got = spline_r2(SPLINE_HUMP, level);
        assert!(
            (got - expected).abs() < 2e-5,
            "{level}: {got} against {expected}"
        );
    }
}

#[test]
fn spline_ordinal_levels_equal_the_best_nonnegative_fit_on_the_monotone_basis() {
    // R: every subset of the monotone spline columns fitted by lm(y ~ columns + z), the best
    // R square among the subsets whose spline coefficients are all nonnegative (the same
    // basis as above, each column the sum of a B-spline and all later ones).
    let cases = [
        ("SPORD DEGREE=1 INKNOT=2", 0.781492),
        ("SPORD DEGREE=2 INKNOT=1", 0.780719),
        ("SPORD DEGREE=2 INKNOT=2", 0.778847),
        ("SPORD DEGREE=3 INKNOT=1", 0.779698),
    ];
    for (level, expected) in cases {
        let got = spline_r2(SPLINE_RISING, level);
        assert!(
            (got - expected).abs() < 2e-5,
            "{level}: {got} against {expected}"
        );
    }
}

/// Eighty invented cases: a fractional predictor x (one decimal, with ties), a covariate z in 6
/// bands and a response that rises with both.
const CONTINUOUS: &str = "DATA LIST FREE /y x z.
BEGIN DATA
7 52.6 5  4 55.8 1  4 51.1 1  5 55.3 2  3 45.6 3  3 51.5 2
5 39.6 5  7 55.9 5  6 45.6 2  2 34.5 1  5 41.1 3  8 61.1 4
5 59.0 1  2 19.9 6  2 13.5 3  8 50.0 5  2 45.3 2  1 29.1 1
4 56.0 3  3 53.3 1  6 63.2 6  4 59.0 2  5 49.3 5  4 45.9 2
6 52.7 4  6 56.6 4  6 58.2 3  3 43.4 2  2 33.6 3  5 66.8 4
7 66.5 5  6 55.4 2  3 48.2 2  3 51.5 3  5 22.5 6  4 33.6 3
3 47.6 1  5 50.8 4  4 51.1 2  7 53.9 4  4 51.6 1  3 29.8 2
2 46.7 1  3 31.4 2  4 33.4 4  5 33.7 6  4 38.9 5  5 39.6 2
7 69.9 5  3 48.1 2  3 31.1 1  7 57.5 6  5 54.0 1  4 45.2 5
4 37.0 5  4 49.1 4  5 43.7 5  7 54.7 5  3 41.8 1  4 41.0 1
6 48.0 6  4 48.6 3  6 44.7 6  6 53.1 2  5 36.8 6  4 34.0 4
4 52.5 6  5 50.4 6  3 44.7 2  4 54.8 2  5 57.0 6  5 55.8 2
4 40.3 5  4 53.6 4  4 40.1 5  6 51.7 3  6 58.9 5  6 66.6 6
4 42.4 4  6 52.9 4
END DATA.
";

#[test]
fn discretization_gives_the_same_fit_as_the_recoding_written_in_r() {
    // R: the codes of x, then lm(y ~ factor(code) + z) for the nominal level of x. Normal
    // grouping cuts the z score at the optimal quantizer of the normal (fixed point iteration,
    // integrate()); ranking is rank(x, ties.method = "min"); multiplying is round(10 * z score);
    // uniform grouping fills groups to round(n / k) cases from whole values; equal intervals
    // are floor((x - min) / width) + 1. The iterations stop at 1e-5 of the loss.
    let cases = [
        ("GROUPING NCAT=5 DISTR=NORMAL", 0.570862),
        ("GROUPING NCAT=7", 0.559270),
        ("RANKING", 0.942707),
        ("MULTIPLYING", 0.685971),
        ("GROUPING NCAT=5 DISTR=UNIFORM", 0.576062),
        ("GROUPING EQINTV=5", 0.582827),
    ];
    for (how, expected) in cases {
        let mut engine = Engine::default();
        let t = run(
            &mut engine,
            &format!(
                "{CONTINUOUS}CATREG VARIABLES=y x z /ANALYSIS=y(LEVEL=NUME) WITH x(LEVEL=NOMI) \
                 z(LEVEL=NUME) /DISCRETIZATION=x({how}) /INITIAL=NUMERICAL /PRINT=R."
            ),
        );
        assert!(
            (r2(&t) - expected).abs() < 1e-4,
            "{how}: {} against {expected}",
            r2(&t)
        );
    }
}
