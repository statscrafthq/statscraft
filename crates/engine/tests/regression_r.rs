//! REGRESSION diagnostics against R's lm on 40 invented cases (one outlier). The saved values
//! are compared at four cases (the first, the outlier 7, 20 and 38) with R's fitted, resid,
//! rstandard, rstudent, hatvalues, cooks.distance, dfbeta, dfbetas, dffits and covratio, in the
//! definitions of the manual (leverage centered, Mahalanobis (n - 1) times it, the standard
//! error of the prediction s sqrt(h), the intervals of the mean and of one case). The tables
//! come from R written out: summary(lm), the Durbin-Watson sum, the part and partial
//! correlations from the t values and the reduced fits, tolerance and VIF from the
//! regressions of each predictor on the others, and the collinearity diagnostics from the
//! eigenvalues of X'X scaled to unit columns (the variance proportions from the eigenvectors).
//! The last tests cover regression weights (REGWGT: lm with weights, the weighted diagnostics in
//! the manual's own formulas, which differ from R's where the manual leaves out the weight),
//! mean substitution (lm on the filled-in data), pairwise deletion (the normal equations from
//! the pairwise correlations with each variable's own mean and deviation) and regression
//! through the origin (lm without an intercept).

use statscraft_data::Values;
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /y x1 x2 x3.
BEGIN DATA
14.87 8.9 6.23 6.13 
16.48 12.18 7.89 4.9 
11.45 11.28 7.04 4.24 
16.25 12.09 8.11 4.76 
9.69 10.34 8.3 4.86 
15.12 12.28 8.41 5.71 
20.49 8.06 4.5 5.2 
14.44 9.74 7.62 6.08 
13.24 10.29 7.67 5 
11.78 12.88 9.67 1.74 
10.41 4.12 1.32 6.32 
13.37 9.51 7.72 6.25 
9.6 9.72 9.61 4.93 
6.03 9.93 7.55 -0.11 
11.99 10.56 4.67 2.73 
15.88 11.18 5.39 5.38 
14.3 12.05 8.27 4.29 
17.5 14.21 8.8 6.47 
18.69 10.31 9.11 8.65 
12.27 11.83 6.78 4.88 
10.5 9.49 7.12 4.03 
9.11 13.04 9.16 2.57 
15.39 13.56 8.29 5.19 
12.67 8.24 5.78 3.95 
8.98 6.94 4.66 2.56 
10.74 10.27 7.7 4.54 
13.57 8.58 5.5 5.44 
11.34 7.18 4.46 6.68 
12.13 13.66 9.77 3.84 
14.77 12.58 11.02 5.72 
14.23 5.28 1.73 6.09 
12.93 8.9 5.83 5.19 
11.24 9.39 7.11 3.46 
11.58 8.5 6.28 3.97 
14.74 10.29 6.9 4.81 
13.58 8.9 9.05 5.02 
11.66 10.32 6.97 3.71 
11.75 9.82 6.93 7.12 
13.61 10.16 4.42 4.82 
16.24 11.8 9.36 6.79 
END DATA.
";

fn run() -> (Engine, Vec<Table>) {
    let source = format!(
        "{DATA}REGRESSION /STATISTICS=COEFF OUTS R ANOVA CI ZPP COLLIN TOL BCOV CHANGE /DEPENDENT=y\n\
         /METHOD=ENTER x1 x2 x3 /RESIDUALS=DURBIN OUTLIERS(ZRESID)\n\
         /SAVE=PRED(p) RESID(e) ZPRED(zp) ZRESID(ze) SRESID(sr) DRESID(dr) SDRESID(sdr) ADJPRED(ap)\n\
         SEPRED(sep) MAHAL(mh) COOK(ck) LEVER(lv) DFBETA(dfb) SDBETA(sdb) DFFIT(dff) SDFIT(sdf)\n\
         COVRATIO(cr) MCIN ICIN."
    );
    let (commands, errors) = parse(&source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        tables.extend(engine.execute(&command.command).expect("runs"));
    }
    (engine, tables)
}

fn near(got: f64, want: f64, what: &str) {
    assert!(
        (got - want).abs() <= 1e-8 * want.abs().max(1e-3),
        "{what}: {got} vs {want}"
    );
}

fn saved(engine: &Engine, name: &str) -> Vec<f64> {
    let data = engine.dataset().unwrap();
    let Values::Numeric(v) = data.variable(name).unwrap().values() else {
        panic!("{name} is not numeric")
    };
    (0..data.case_count()).map(|c| v.get(c).unwrap()).collect()
}

#[test]
fn the_saved_diagnostics_agree_with_r() {
    let (engine, _) = run();
    let cases = [0usize, 6, 19, 37];
    let expected: [(&str, [f64; 4]); 27] = [
        ("p", [14.05975987, 13.10031595, 14.55107343, 15.64608335]),
        ("e", [0.8102401251, 7.389684052, -2.281073426, -3.896083346]),
        (
            "zp",
            [0.4580382292, -0.007242239702, 0.6962997817, 1.227322715],
        ),
        (
            "ze",
            [0.3993498569, 3.642215656, -1.124291822, -1.920295328],
        ),
        ("sr", [0.4093444871, 3.765263138, -1.169128825, -2.00003052]),
        ("dr", [0.8513038005, 7.897419759, -2.466640728, -4.22634975]),
        (
            "sdr",
            [0.404561727, 4.768416513, -1.175304692, -2.091685975],
        ),
        ("ap", [14.0186962, 12.59258024, 14.73664073, 15.97634975]),
        (
            "sep",
            [0.4456016974, 0.5144420446, 0.556490803, 0.5671653559],
        ),
        ("mh", [0.906212487, 1.532362297, 1.959000357, 2.072639332]),
        (
            "ck",
            [0.002123058552, 0.2435244154, 0.02779885659, 0.08477141111],
        ),
        (
            "lv",
            [0.02323621761, 0.03929134094, 0.05023077838, 0.05314459827],
        ),
        (
            "dfb0",
            [0.02333653306, 0.9541819715, 0.2440436322, 0.3839683744],
        ),
        (
            "dfb1",
            [
                -0.005783713947,
                -0.001844520066,
                -0.066001816,
                0.0001223096923,
            ],
        ),
        (
            "dfb2",
            [0.001058199069, -0.116829018, 0.05787797935, -0.001601210403],
        ),
        (
            "dfb3",
            [0.0102136405, 0.01810598988, -0.008447162076, -0.09892408298],
        ),
        (
            "sdb0",
            [0.01157774064, 0.6065989982, 0.1231536289, 0.2015797712],
        ),
        (
            "sdb1",
            [
                -0.02348287707,
                -0.009596456199,
                -0.2725791528,
                0.0005254959648,
            ],
        ),
        (
            "sdb2",
            [0.004147655806, -0.5867715686, 0.2307495952, -0.006641218208],
        ),
        (
            "sdb3",
            [0.04845088526, 0.1100592185, -0.04075906656, -0.4965776799],
        ),
        (
            "dff",
            [0.04106367538, 0.5077357062, -0.185567302, -0.3302664033],
        ),
        (
            "sdf",
            [0.09107660433, 1.249913909, -0.3352212676, -0.6089961728],
        ),
        ("cr", [1.154261038, 0.1615208333, 1.036720816, 0.7579809724]),
        (
            "LMCI_1",
            [13.15603775, 12.05697912, 13.42245777, 14.49581869],
        ),
        ("UMCI_1", [14.963482, 14.14365277, 15.67968909, 16.796348]),
        (
            "LICI_1",
            [9.846891655, 8.855307504, 10.28430436, 11.37353683],
        ),
        (
            "UICI_1",
            [18.2726281, 17.34532439, 18.81784249, 19.91862986],
        ),
    ];
    for (name, want) in expected {
        let got = saved(&engine, name);
        assert_eq!(got.len(), 40, "{name}");
        for (case, want) in cases.iter().zip(want) {
            near(got[*case], want, &format!("{name}[{case}]"));
        }
    }
}

/// The number in `column` of row `row` of the table `title`.
fn at(tables: &[Table], title: &str, row: usize, column: &str) -> f64 {
    let t = tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"));
    let c = t
        .column_titles()
        .iter()
        .position(|c| c == column || c.ends_with(&format!("/ {column}")))
        .unwrap_or_else(|| panic!("no column {column} in {:?}", t.column_titles()));
    match t.rows[row][c] {
        Cell::Number(n) => n,
        ref other => panic!("{title} row {row} {column}: {other:?}"),
    }
}

#[test]
fn the_tables_agree_with_r() {
    let (_, tables) = run();
    let summary = "Model Summary";
    for (column, want) in [
        ("R", 0.7266989858),
        ("R Square", 0.5280914159),
        ("Adjusted R Square", 0.4887657006),
        ("Std. Error of the Estimate", 2.028897998),
        ("F Change", 13.42865378),
        ("Durbin-Watson", 1.977666216),
    ] {
        near(at(&tables, summary, 0, column), want, column);
    }
    near(
        at(&tables, "ANOVA", 0, "Sum of Squares"),
        165.8342224,
        "SS regression",
    );
    near(
        at(&tables, "ANOVA", 1, "Sum of Squares"),
        148.1913751,
        "SS residual",
    );
    // the constant and x1, x2, x3: B, standard error, t, interval, correlations, collinearity
    let coefficients = "Coefficients";
    let (lower, upper) = ("Lower Bound", "Upper Bound");
    for (row, b, se, ci) in [
        (
            1,
            0.776873510686,
            0.243417248717,
            (0.2832004488, 1.270546573),
        ),
        (
            2,
            -0.477334818028,
            0.252150895257,
            (-0.988720536, 0.03405089998),
        ),
        (
            3,
            1.217913347851,
            0.208340978432,
            (0.7953782593, 1.640448436),
        ),
    ] {
        near(at(&tables, coefficients, row, "B"), b, "B");
        near(at(&tables, coefficients, row, "Std. Error"), se, "SE");
        near(at(&tables, coefficients, row, lower), ci.0, "lower");
        near(at(&tables, coefficients, row, upper), ci.1, "upper");
    }
    for (row, beta, zero, partial, part, tolerance, vif) in [
        (
            1,
            0.5966970387,
            0.2265526642,
            0.4696176617,
            0.3654068229,
            0.3750123234,
            2.666579036,
        ),
        (
            2,
            -0.3512561934,
            0.07301318322,
            -0.3008878827,
            -0.2167406083,
            0.3807435849,
            2.626439524,
        ),
        (
            3,
            0.6775603781,
            0.6177374864,
            0.6978411931,
            0.6692977716,
            0.9757594245,
            1.024842779,
        ),
    ] {
        near(at(&tables, coefficients, row, "Beta"), beta, "beta");
        near(
            at(&tables, coefficients, row, "Zero-order"),
            zero,
            "zero-order",
        );
        near(
            at(&tables, coefficients, row, "Partial"),
            partial,
            "partial",
        );
        near(at(&tables, coefficients, row, "Part"), part, "part");
        near(
            at(&tables, coefficients, row, "Tolerance"),
            tolerance,
            "tolerance",
        );
        near(at(&tables, coefficients, row, "VIF"), vif, "VIF");
    }
    // the coefficient correlations (rows 0 to 2) and covariances (rows 3 to 5)
    let bcov = "Coefficient Correlations";
    near(at(&tables, bcov, 0, "x2"), -0.7858769598, "corr x1 x2");
    near(at(&tables, bcov, 0, "x3"), 0.1414291776, "corr x1 x3");
    near(at(&tables, bcov, 3, "x1"), 0.05925195697, "var x1");
    near(at(&tables, bcov, 4, "x3"), -0.003723960341, "cov x2 x3");
    // the collinearity diagnostics: eigenvalues, condition indices, variance proportions
    let collin = "Collinearity Diagnostics";
    for (row, eigenvalue, index) in [
        (0, 3.853594585, 1.0),
        (1, 0.1083163161, 5.964665193),
        (2, 0.02815349613, 11.69948769),
        (3, 0.009935602993, 19.69408902),
    ] {
        near(
            at(&tables, collin, row, "Eigenvalue"),
            eigenvalue,
            "eigenvalue",
        );
        near(
            at(&tables, collin, row, "Condition Index"),
            index,
            "condition index",
        );
    }
    for (column, first, last) in [
        ("(Constant)", 0.001715105297, 0.4854378174),
        ("x1", 0.001053096093, 0.9705061539),
        ("x2", 0.001937664471, 0.6001061486),
        ("x3", 0.005712093266, 0.09212467336),
    ] {
        near(at(&tables, collin, 0, column), first, column);
        near(at(&tables, collin, 3, column), last, column);
    }
    // the outliers by standardized residual: case 7 is the largest
    let outliers = "Outlier Statistics";
    near(at(&tables, outliers, 0, "Case Number"), 7.0, "outlier case");
    near(
        at(&tables, outliers, 0, "Statistic"),
        3.642215656,
        "largest standardized residual",
    );
    near(
        at(&tables, outliers, 1, "Case Number"),
        38.0,
        "second outlier case",
    );
}

/// Forty cases with a regression weight g.
const WEIGHTED: &str = "DATA LIST FREE /y x1 x2 x3 g.
BEGIN DATA
14.87 8.9 6.23 6.13 3
16.48 12.18 7.89 4.9 1.5
11.45 11.28 7.04 4.24 1.5
16.25 12.09 8.11 4.76 3
9.69 10.34 8.3 4.86 0.5
15.12 12.28 8.41 5.71 2
20.49 8.06 4.5 5.2 1
14.44 9.74 7.62 6.08 0.5
13.24 10.29 7.67 5 1
11.78 12.88 9.67 1.74 0.5
10.41 4.12 1.32 6.32 1.5
13.37 9.51 7.72 6.25 2
9.6 9.72 9.61 4.93 1
6.03 9.93 7.55 -0.11 2
11.99 10.56 4.67 2.73 3
15.88 11.18 5.39 5.38 0.5
14.3 12.05 8.27 4.29 3
17.5 14.21 8.8 6.47 1
18.69 10.31 9.11 8.65 0.5
12.27 11.83 6.78 4.88 1
10.5 9.49 7.12 4.03 2
9.11 13.04 9.16 2.57 1.5
15.39 13.56 8.29 5.19 1
12.67 8.24 5.78 3.95 2
8.98 6.94 4.66 2.56 1
10.74 10.27 7.7 4.54 0.5
13.57 8.58 5.5 5.44 1
11.34 7.18 4.46 6.68 3
12.13 13.66 9.77 3.84 3
14.77 12.58 11.02 5.72 2
14.23 5.28 1.73 6.09 1
12.93 8.9 5.83 5.19 1
11.24 9.39 7.11 3.46 0.5
11.58 8.5 6.28 3.97 0.5
14.74 10.29 6.9 4.81 1
13.58 8.9 9.05 5.02 1
11.66 10.32 6.97 3.71 1
11.75 9.82 6.93 7.12 1
13.61 10.16 4.42 4.82 1.5
16.24 11.8 9.36 6.79 1.5
END DATA.
";

/// The same cases with -99 (user-missing) in seven of them: x1 three times, x2 and y and x3 once
/// or twice.
const MISSING: &str = "DATA LIST FREE /y x1 x2 x3 g.
MISSING VALUES y x1 x2 x3 (-99).
BEGIN DATA
14.87 8.9 6.23 6.13 3
16.48 12.18 7.89 4.9 1.5
11.45 11.28 7.04 4.24 1.5
16.25 -99 8.11 4.76 3
9.69 10.34 8.3 4.86 0.5
15.12 12.28 8.41 5.71 2
20.49 8.06 4.5 5.2 1
14.44 9.74 7.62 6.08 0.5
13.24 10.29 7.67 5 1
11.78 12.88 -99 1.74 0.5
10.41 4.12 1.32 6.32 1.5
13.37 9.51 7.72 6.25 2
9.6 9.72 9.61 4.93 1
6.03 9.93 7.55 -0.11 2
11.99 -99 4.67 2.73 3
15.88 11.18 5.39 5.38 0.5
14.3 12.05 8.27 4.29 3
17.5 14.21 8.8 6.47 1
18.69 10.31 9.11 8.65 0.5
12.27 11.83 6.78 4.88 1
-99 9.49 7.12 4.03 2
9.11 13.04 9.16 2.57 1.5
15.39 13.56 8.29 5.19 1
12.67 8.24 5.78 3.95 2
8.98 6.94 4.66 2.56 1
10.74 10.27 7.7 4.54 0.5
13.57 8.58 -99 5.44 1
11.34 7.18 4.46 6.68 3
12.13 13.66 9.77 3.84 3
14.77 12.58 11.02 5.72 2
14.23 5.28 1.73 6.09 1
12.93 8.9 5.83 -99 1
11.24 9.39 7.11 3.46 0.5
11.58 8.5 6.28 3.97 0.5
14.74 10.29 6.9 4.81 1
13.58 -99 9.05 5.02 1
11.66 10.32 6.97 3.71 1
11.75 9.82 6.93 7.12 1
13.61 10.16 4.42 4.82 1.5
16.24 11.8 9.36 6.79 1.5
END DATA.
";

fn run_on(data: &str, syntax: &str) -> (Engine, Vec<Table>) {
    let (commands, errors) = parse(&format!("{data}{syntax}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        tables.extend(engine.execute(&command.command).expect("runs"));
    }
    (engine, tables)
}

/// The Model Summary, ANOVA and Coefficients tables against R: R, R square, adjusted R square,
/// standard error, F, its significance and the coefficients (B, standard error, t, significance).
fn check_fit(tables: &[Table], summary: [f64; 4], f: (f64, f64), coefficients: &[[f64; 4]]) {
    near(
        at(tables, "Model Summary", 0, "R Square"),
        summary[0],
        "R square",
    );
    near(
        at(tables, "Model Summary", 0, "Adjusted R Square"),
        summary[1],
        "adjusted R square",
    );
    near(
        at(tables, "Model Summary", 0, "Std. Error of the Estimate"),
        summary[2],
        "standard error",
    );
    near(at(tables, "Model Summary", 0, "R"), summary[3], "R");
    near(at(tables, "ANOVA", 0, "F"), f.0, "F");
    near(at(tables, "ANOVA", 0, "Sig."), f.1, "F significance");
    for (row, want) in coefficients.iter().enumerate() {
        near(
            at(tables, "Coefficients", row, "B"),
            want[0],
            &format!("B {row}"),
        );
        near(
            at(tables, "Coefficients", row, "Std. Error"),
            want[1],
            &format!("se {row}"),
        );
        near(
            at(tables, "Coefficients", row, "t"),
            want[2],
            &format!("t {row}"),
        );
        near(
            at(tables, "Coefficients", row, "Sig."),
            want[3],
            &format!("p {row}"),
        );
    }
}

#[test]
fn regression_weights_agree_with_weighted_lm_and_the_manuals_diagnostics() {
    let (engine, tables) = run_on(
        WEIGHTED,
        "REGRESSION /REGWGT=g /STATISTICS=COEFF OUTS R ANOVA /DEPENDENT=y /METHOD=ENTER x1 x2 x3\n\
         /RESIDUALS=DURBIN\n\
         /SAVE=PRED(p) RESID(e) ZPRED(zp) ZRESID(ze) SRESID(sr) DRESID(dr) SDRESID(sdr) ADJPRED(ap)\n\
         SEPRED(sep) MAHAL(mh) COOK(ck) LEVER(lv) DFBETA(dfb) SDBETA(sdb) DFFIT(dff) SDFIT(sdf)\n\
         COVRATIO(cr).",
    );
    // summary(lm(y ~ x1 + x2 + x3, weights = g)): R square 0.5474769698, F 14.51798737
    check_fit(
        &tables,
        [
            0.5474769698,
            0.5097667173,
            2.26570054,
            0.5474769698_f64.sqrt(),
        ],
        (14.51798737, 2.338055690e-06),
        &[
            [1.8169197912, 1.9312890695, 0.9407808597, 0.3530874725],
            [0.8449927598, 0.2404753006, 3.5138442811, 0.001211216278],
            [-0.4558914790, 0.2483740072, -1.8355039814, 0.07470288013],
            [1.2045471027, 0.1978861377, 6.0870716688, 5.317403589e-07],
        ],
    );
    // Durbin-Watson of the residuals times the square root of the regression weight
    near(
        at(&tables, "Model Summary", 0, "Durbin-Watson"),
        1.904010104744,
        "Durbin-Watson",
    );
    // the saved values at cases 1, 7, 20 and 38: R's fitted, resid, rstandard, hatvalues,
    // cooks.distance and dfbeta, and the manual's formulas for the rest
    let expected: [(&str, [f64; 4]); 14] = [
        ("p", [13.88102518, 12.83969471, 14.60042977, 15.53179611]),
        ("e", [0.9889748212, 7.650305286, -2.330429773, -3.781796114]),
        ("sr", [0.7994577876, 3.453280573, -1.056589307, -1.71915491]),
        ("dr", [1.105833142, 8.001842173, -2.459129609, -4.011778336]),
        (
            "sdr",
            [0.4564640235, 4.163744414, -1.05835037, -1.769291535],
        ),
        ("ap", [13.76416686, 12.48815783, 14.72912961, 15.76177834]),
        (
            "sep",
            [0.4252329411, 0.474889939, 0.518323376, 0.5424768091],
        ),
        ("mh", [2.068672375, 1.029137261, 1.356874901, 1.551532807]),
        (
            "ck",
            [0.01888015211, 0.1369922112, 0.01541325648, 0.04493307983],
        ),
        (
            "lv",
            [0.05304288141, 0.0263881349, 0.03479166413, 0.03978289248],
        ),
        (
            "dfb1",
            [
                -0.01807431361,
                -0.003350707525,
                -0.04842542716,
                -0.001387772693,
            ],
        ),
        (
            "sdb1",
            [-0.0743298029, -0.01680034663, -0.201709447, -0.005939258791],
        ),
        (
            "dff",
            [0.1168583205, 0.3515368867, -0.1286998361, -0.2299822223],
        ),
        ("cr", [1.222168861, 0.2341497154, 1.041260421, 0.8428760546]),
    ];
    for (name, want) in expected {
        let got = saved(&engine, name);
        for (case, want) in [0usize, 6, 19, 37].iter().zip(want) {
            near(got[*case], want, &format!("{name}[{case}]"));
        }
    }
    let sdf = saved(&engine, "sdf");
    for (case, want) in
        [0usize, 6, 19, 37]
            .iter()
            .zip([0.1569075195, 0.8925450199, -0.2487141265, -0.4363123208])
    {
        near(sdf[*case], want, &format!("sdf[{case}]"));
    }
    // the standardized predicted value and residual are not defined with a regression weight
    for name in ["zp", "ze"] {
        let data = engine.dataset().unwrap();
        let Values::Numeric(v) = data.variable(name).unwrap().values() else {
            panic!("{name} is not numeric")
        };
        assert!((0..data.case_count()).all(|c| v.get(c).is_none()), "{name}");
    }
}

#[test]
fn mean_substitution_pairwise_deletion_and_the_origin_agree_with_r() {
    // lm on the data with each variable's missing values replaced by its mean of the valid ones
    let (_, tables) = run_on(
        MISSING,
        "REGRESSION /MISSING=MEANSUBSTITUTION /DEPENDENT=y /METHOD=ENTER x1 x2 x3 /STATISTICS=COEFF R ANOVA.",
    );
    check_fit(
        &tables,
        [0.5009755547, 0.4593901843, 2.062939114, 0.7077962664],
        (12.04691817, 1.306336456e-05),
        &[
            [3.2032782453, 2.0399365853, 1.570283247, 0.1250981195],
            [0.7203026705, 0.2478828480, 2.905818923, 0.006229603283],
            [-0.4639972833, 0.2599983108, -1.784616530, 0.08275317330],
            [1.2213520172, 0.2143079771, 5.699050654, 1.754876375e-06],
        ],
    );
    // pairwise deletion: each variable's own mean and deviation, the correlations from the
    // cases that have both, N the smallest of the pair counts (35)
    let (_, tables) = run_on(
        MISSING,
        "REGRESSION /MISSING=PAIRWISE /DEPENDENT=y /METHOD=ENTER x1 x2 x3 /STATISTICS=COEFF R ANOVA.",
    );
    check_fit(
        &tables,
        [
            0.5382601316,
            0.4935756282,
            2.022750193,
            0.5382601316_f64.sqrt(),
        ],
        (12.04578972, 2.150083408e-05),
        &[
            [2.607707331, 2.116463865, 1.2321057657, 0.2271735590],
            [0.8982184582, 0.2815546378, 3.190210133, 0.003246350819],
            [-0.6690563931, 0.2992156390, -2.236034170, 0.03268634113],
            [1.268332429, 0.2239341803, 5.663862605, 3.202573722e-06],
        ],
    );
    // through the origin, on the 33 complete cases: lm(y ~ 0 + x1 + x2 + x3), the R square
    // from the uncentered total
    let (_, tables) = run_on(
        MISSING,
        "REGRESSION /ORIGIN /DEPENDENT=y /METHOD=ENTER x1 x2 x3 /STATISTICS=COEFF R ANOVA.",
    );
    check_fit(
        &tables,
        [
            0.9770132185,
            0.9747145404,
            2.144471556,
            0.9770132185_f64.sqrt(),
        ],
        (425.0326303, 1.171020160e-24),
        &[
            [1.0269881589, 0.2381517040, 4.312327571, 0.0001608323506],
            [-0.6512086441, 0.3064013642, -2.125345120, 0.04190588603],
            [1.4375195254, 0.1904225038, 7.549105261, 2.039684905e-08],
        ],
    );
}

#[test]
fn residual_statistics_casewise_and_outlier_tables_agree_with_r() {
    // the cases of WEIGHTED fitted without the weight, g labelling the cases
    let (_, tables) = run_on(
        WEIGHTED,
        "REGRESSION /DEPENDENT=y /METHOD=ENTER x1 x2 x3\n\
         /RESIDUALS=OUTLIERS(ZRESID COOK) ID(g) /CASEWISE=PLOT(ZRESID) OUTLIERS(1.5) DEPENDENT PRED RESID COOK.",
    );
    // minimum, maximum, mean and deviation of fitted, scale(fitted), resid, resid / sigma and
    // cooks.distance; the means are zero up to rounding
    let rows = [
        (
            "Predicted Value",
            [6.63007834051, 17.3722982726, 13.11525, 2.06207651387],
        ),
        (
            "Std. Predicted Value",
            [-3.1449713994, 2.06444729085, 0.0, 1.0],
        ),
        (
            "Residual",
            [-3.89608334648, 7.3896840524, 0.0, 1.94930214394],
        ),
        (
            "Std. Residual",
            [-1.92029532809, 3.64221565607, 0.0, 0.960768922831],
        ),
        (
            "Cook's Distance",
            [
                7.68777286669e-07,
                0.243524415402,
                0.0260732391651,
                0.0495032009625,
            ],
        ),
    ];
    let t = tables
        .iter()
        .find(|t| t.title == "Residuals Statistics")
        .expect("table");
    for (label, want) in rows {
        let row = t
            .rows
            .iter()
            .find(|r| matches!(&r[0], Cell::Text(s) if s == label))
            .unwrap_or_else(|| panic!("no row {label}"));
        for (c, want) in want.iter().enumerate() {
            let Cell::Number(got) = row[c + 1] else {
                panic!("{label} {c}")
            };
            if *want == 0.0 {
                assert!(got.abs() < 1e-12, "{label} column {c}: {got}");
            } else {
                near(got, *want, &format!("{label} column {c}"));
            }
        }
        let Cell::Number(n) = row[5] else {
            panic!("N of {label}")
        };
        near(n, 40.0, "N");
    }
    // the cases with |standardized residual| >= 1.5, by case number
    let casewise = tables
        .iter()
        .find(|t| t.title == "Casewise Diagnostics")
        .expect("casewise");
    assert_eq!(casewise.rows.len(), 2);
    near(
        at(&tables, "Casewise Diagnostics", 0, "Case Number"),
        7.0,
        "case",
    );
    near(
        at(&tables, "Casewise Diagnostics", 0, "Std. Residual"),
        3.64221565607,
        "zresid 7",
    );
    near(
        at(&tables, "Casewise Diagnostics", 0, "Cook's Distance"),
        0.243524415402,
        "cook 7",
    );
    near(
        at(&tables, "Casewise Diagnostics", 1, "Case Number"),
        38.0,
        "case",
    );
    near(
        at(&tables, "Casewise Diagnostics", 1, "Std. Residual"),
        -1.92029532809,
        "zresid 38",
    );
    // the ten largest |standardized residual| and the largest Cook's distances
    let cases = [7.0, 38.0, 5.0, 28.0, 11.0, 22.0, 20.0, 4.0, 36.0, 2.0];
    let z = [
        3.64221565607,
        -1.92029532809,
        -1.45577782042,
        -1.4284958676,
        -1.23782565169,
        -1.19850957343,
        -1.1242918219,
        1.12274957778,
        1.09330687417,
        1.06585162394,
    ];
    for (row, (case, z)) in cases.iter().zip(z).enumerate() {
        near(
            at(&tables, "Outlier Statistics", row, "Case Number"),
            *case,
            "outlier case",
        );
        near(
            at(&tables, "Outlier Statistics", row, "Statistic"),
            z,
            "outlier z",
        );
    }
    for (offset, (case, cook)) in [
        (7.0, 0.243524415402),
        (11.0, 0.176751898353),
        (38.0, 0.0847714111061),
    ]
    .iter()
    .enumerate()
    {
        near(
            at(&tables, "Outlier Statistics", 10 + offset, "Case Number"),
            *case,
            "cook case",
        );
        near(
            at(&tables, "Outlier Statistics", 10 + offset, "Statistic"),
            *cook,
            "cook",
        );
    }
}
