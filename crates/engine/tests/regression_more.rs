//! PROBIT, CURVEFIT and WLS on invented data. The expected numbers come from an independent
//! general statistics package: binomial models with probit and logit links (standard errors
//! from a numerical Hessian of the log-likelihood, fiducial limits by solving Fieller's
//! quadratic directly), least squares on each curve's linear form, and weighted least squares
//! with the log-likelihood computed by hand.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

/// Insects of two strains given five doses of a spray: how many were exposed and how many
/// were knocked down.
const SPRAY: &str = "DATA LIST LIST /strain dose exposed down (4F4).
BEGIN DATA
1 2 40 4
1 4 42 9
1 8 38 17
1 16 41 28
1 32 39 35
2 2 44 2
2 4 40 5
2 8 43 12
2 16 39 20
2 32 42 33
END DATA.
VALUE LABELS strain 1 'field' 2 'lab'.
";

/// Seeds soaked at six strengths of a growth agent and an untreated batch (strength 0) that
/// sprouts on its own.
const SEEDS: &str = "DATA LIST LIST /strength sown sprouted (3F4).
BEGIN DATA
0 50 6
1 48 7
2 50 9
4 49 14
8 51 24
16 50 37
32 48 45
END DATA.
";

/// Twelve monthly readings of a growing quantity, with an independent x and case weights.
const SERIES: &str = "DATA LIST LIST /y x w (3F6.2).
BEGIN DATA
3.1 1.5 1
3.9 2.0 2
4.6 2.2 1
5.8 3.1 3
7.2 3.5 1
8.1 4.4 2
10.3 5.0 2
12.2 5.2 1
14.9 6.3 1
17.6 7.1 3
21.4 7.7 2
25.8 8.4 1
END DATA.
";

/// Shop running costs, floor area and staff; the spread of the costs grows with the area.
const SHOPS: &str = "DATA LIST LIST /cost area staff (3F6.2).
BEGIN DATA
12.1 1.2 2
15.8 1.5 3
14.2 1.8 2
19.9 2.1 4
22.5 2.4 3
21.0 2.9 5
30.2 3.3 4
26.1 3.6 6
35.7 4.0 5
41.9 4.4 7
33.0 4.9 6
49.8 5.3 8
44.1 5.8 7
61.5 6.2 9
52.3 6.9 8
END DATA.
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

fn error(data: &str, source: &str) -> String {
    let (commands, errors) = parse(&format!("{data}{source}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    for command in &commands {
        if let Err(e) = engine.execute(&command.command) {
            return e.to_string();
        }
    }
    panic!("no error")
}

/// The `nth` table (from 0) with this title.
fn find<'a>(tables: &'a [Table], title: &str, nth: usize) -> &'a Table {
    tables
        .iter()
        .filter(|t| t.title == title)
        .nth(nth)
        .unwrap_or_else(|| {
            panic!(
                "no table {title} #{nth}: {:?}",
                tables.iter().map(|t| &t.title).collect::<Vec<_>>()
            )
        })
}

fn num(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(v) => *v,
        other => panic!("not a number: {other:?}"),
    }
}

fn close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance * expected.abs().max(1.0),
        "{actual} vs {expected}"
    );
}

/// The row whose first cells are these texts.
fn row<'a>(table: &'a Table, heads: &[&str]) -> &'a [Cell] {
    table
        .rows
        .iter()
        .find(|r| {
            heads
                .iter()
                .zip(r.iter())
                .all(|(h, c)| matches!(c, Cell::Text(t) if t == h))
        })
        .unwrap_or_else(|| panic!("no row {heads:?} in {}: {:?}", table.title, table.rows))
}

fn values(run: &Run, name: &str) -> Vec<Option<f64>> {
    let data = run.engine.dataset().expect("data");
    data.variable(name)
        .unwrap()
        .numeric()
        .unwrap()
        .iter()
        .collect()
}

#[test]
fn probit_two_strains_both_links() {
    let r = run(
        SPRAY,
        "PROBIT down OF exposed BY strain(1,2) WITH dose /MODEL=BOTH /PRINT=ALL.",
    );
    // logit first, then probit
    let expected = [
        // slope, intercepts, their standard errors (observed information), Pearson chi-square
        (
            3.56516972993,
            [-3.41460862702, -4.15452445924],
            [0.355403312198, 0.375523136083, 0.416566998865],
            0.514013191371,
        ),
        (
            2.09655996831,
            [-2.00214601763, -2.43111401164],
            [0.191525602632, 0.203561110218, 0.220692115827],
            0.767753497432,
        ),
    ];
    for (i, (slope, intercepts, ses, chi)) in expected.iter().enumerate() {
        let t = find(&r.tables, "Parameter Estimates", i);
        let dose = row(t, &["dose"]);
        close(num(&dose[1]), *slope, 1e-6);
        close(num(&dose[2]), ses[0], 1e-5);
        for (g, label) in ["field", "lab"].iter().enumerate() {
            let a = row(t, &[&format!("Intercept, strain = {label}")]);
            close(num(&a[1]), intercepts[g], 1e-6);
            close(num(&a[2]), ses[g + 1], 1e-5);
        }
        let tests = find(&r.tables, "Chi-Square Tests", i);
        let pearson = row(tests, &["Pearson Goodness-of-Fit Test"]);
        close(num(&pearson[1]), *chi, 1e-6);
        assert_eq!(num(&pearson[2]), 7.0);
    }
    assert!(
        find(&r.tables, "Parameter Estimates", 0)
            .caption
            .as_deref()
            .unwrap()
            .starts_with("LOGIT")
    );
    // probit: observed and expected responses
    let cells = find(&r.tables, "Cell Counts and Residuals", 1);
    close(num(&cells.rows[0][5]), 3.40738328602, 1e-6);
    close(num(&cells.rows[9][6]), 0.843636644055, 1e-5);
    close(num(&cells.rows[3][7]), 0.6992914496132, 1e-6);
    // fiducial limits (doses and log doses) for p = 0.1, 0.5 and 0.9 of each strain
    let limits = find(&r.tables, "Confidence Limits", 1);
    let probit_limits = [
        (
            "field",
            0.10,
            [2.2064964807, 1.5157702671, 2.9412108090],
            [0.3437032391, 0.1806333838, 0.4685261533],
        ),
        (
            "field",
            0.50,
            [9.0150306411, 7.2340650060, 11.2926216901],
            [0.9549672072, 0.8593824071, 1.0527947792],
        ),
        (
            "lab",
            0.90,
            [58.9979355624, 42.2504486935, 92.3863500067],
            [1.7708368152, 1.6258313255, 1.9656078094],
        ),
    ];
    for (group, rate, dose, log) in probit_limits {
        let line = limits
            .rows
            .iter()
            .find(|l| {
                matches!(&l[0], Cell::Text(t) if t == group) && (num(&l[1]) - rate).abs() < 1e-9
            })
            .unwrap();
        for k in 0..3 {
            close(num(&line[2 + k]), dose[k], 1e-5);
            close(num(&line[5 + k]), log[k], 1e-5);
        }
    }
    assert_eq!(limits.rows.len(), 70);
    let logit = find(&r.tables, "Confidence Limits", 0);
    let line = logit
        .rows
        .iter()
        .find(|l| matches!(&l[0], Cell::Text(t) if t == "lab") && (num(&l[1]) - 0.1).abs() < 1e-9)
        .unwrap();
    close(num(&line[2]), 3.5400230246, 1e-5);
    close(num(&line[3]), 2.4261238019, 1e-5);
    close(num(&line[4]), 4.7040700370, 1e-5);
    // relative median potency of field against lab
    for (i, (est, lo, hi)) in [
        (0.6200973251, 0.4452118798, 0.8509496486),
        (0.6243014736, 0.4499290026, 0.8523377264),
    ]
    .iter()
    .enumerate()
    {
        let rmp = &find(&r.tables, "Relative Median Potency Estimates", i).rows[0];
        close(num(&rmp[2]), *est, 1e-5);
        close(num(&rmp[3]), *lo, 1e-5);
        close(num(&rmp[4]), *hi, 1e-5);
    }
    // parallelism
    let tests = find(&r.tables, "Chi-Square Tests", 1);
    let par = row(tests, &["Parallelism Test"]);
    close(num(&par[1]), 0.00798093128742, 1e-4);
    assert_eq!(num(&par[2]), 1.0);
    close(num(&par[3]), 0.928814838354, 1e-4);
    assert!(r.warnings.is_empty(), "{:?}", r.warnings);
}

#[test]
fn probit_heterogeneity_factor_and_print_none() {
    let r = run(
        SPRAY,
        "PROBIT down OF exposed BY strain(1,2) WITH dose /CRITERIA=P(1) /PRINT=CI.",
    );
    let limits = find(&r.tables, "Confidence Limits", 0);
    assert!(limits.caption.as_deref().unwrap().contains("heterogeneity"));
    for (rate, log) in [
        (0.1, [0.3437032391, 0.2844270644, 0.3970388687]),
        (0.9, [1.5662311753, 1.5095630328, 1.6295365610]),
    ] {
        let line = limits
            .rows
            .iter()
            .find(|l| {
                matches!(&l[0], Cell::Text(t) if t == "field") && (num(&l[1]) - rate).abs() < 1e-9
            })
            .unwrap();
        for k in 0..3 {
            close(num(&line[5 + k]), log[k], 1e-5);
        }
    }
    assert!(
        !r.tables
            .iter()
            .any(|t| t.title == "Cell Counts and Residuals")
    );
    let r = run(
        SPRAY,
        "PROBIT down OF exposed WITH dose /PRINT=NONE /LOG=NONE.",
    );
    let titles: Vec<&str> = r.tables.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "Data Information",
            "Convergence Information",
            "Parameter Estimates",
            "Covariances and Correlations of Parameter Estimates",
            "Chi-Square Tests"
        ]
    );
}

#[test]
fn probit_natural_response() {
    // estimated: the control batch only starts the rate; it stays out of the fit under the log
    let expected = [
        (
            "PROBIT",
            [-2.746911963470, 2.734874949320, 0.152601899376],
            [0.6310883519770, 0.5198294538200, 0.0465877576671],
            0.266630863261,
        ),
        (
            "LOGIT",
            [-4.597641194334, 4.591167510365, 0.146489506745],
            [1.153743187152, 0.963273925503, 0.049194430460],
            0.396002495715,
        ),
    ];
    for (model, est, se, chi) in expected {
        let r = run(
            SEEDS,
            &format!("PROBIT sprouted OF sown WITH strength /MODEL={model} /NATRES."),
        );
        let t = find(&r.tables, "Parameter Estimates", 0);
        close(num(&row(t, &["Intercept"])[1]), est[0], 1e-4);
        close(num(&row(t, &["strength"])[1]), est[1], 1e-4);
        close(num(&row(t, &["Intercept"])[2]), se[0], 1e-3);
        close(num(&row(t, &["strength"])[2]), se[1], 1e-3);
        let natural = &find(&r.tables, "Natural Response Rate", 0).rows[0];
        close(num(&natural[1]), est[2], 1e-4);
        close(num(&natural[2]), se[2], 1e-3);
        let tests = find(&r.tables, "Chi-Square Tests", 0);
        close(num(&tests.rows[0][1]), chi, 1e-4);
        assert_eq!(num(&tests.rows[0][2]), 3.0);
        let info = find(&r.tables, "Data Information", 0);
        assert_eq!(num(&row(info, &["Valid cases"])[1]), 6.0);
        assert_eq!(num(&row(info, &["Control group"])[1]), 1.0);
        let cov = find(
            &r.tables,
            "Covariances and Correlations of Parameter Estimates",
            0,
        );
        assert_eq!(cov.rows.len(), 3);
    }
    // fixed at 0.1
    let r = run(SEEDS, "PROBIT sprouted OF sown WITH strength /NATRES=0.1.");
    let t = find(&r.tables, "Parameter Estimates", 0);
    close(num(&row(t, &["Intercept"])[1]), -2.24128824741, 1e-5);
    close(num(&row(t, &["strength"])[1]), 2.35586659524, 1e-5);
    close(num(&row(t, &["strength"])[2]), 0.343622537309, 1e-4);
    let tests = find(&r.tables, "Chi-Square Tests", 0);
    close(num(&tests.rows[0][1]), 1.48035160083, 1e-5);
    assert_eq!(num(&tests.rows[0][2]), 4.0);
}

#[test]
fn probit_errors_and_weights() {
    let bad = "DATA LIST LIST /dose n r (3F4).\nBEGIN DATA\n1 10 3\n2 10 12\n4 10 8\nEND DATA.\n";
    assert!(error(bad, "PROBIT r OF n WITH dose.").contains("responses"));
    // a case weight of 2 is the same as doubling both counts
    let doubled = SPRAY.replace("1 8 38 17", "1 8 76 34");
    let weighted = format!("{SPRAY}COMPUTE w = 1 + (strain = 1 AND dose = 8).\nWEIGHT BY w.\n");
    let a = run(
        &weighted,
        "PROBIT down OF exposed BY strain(1,2) WITH dose /PRINT=NONE.",
    );
    let b = run(
        &doubled,
        "PROBIT down OF exposed BY strain(1,2) WITH dose /PRINT=NONE.",
    );
    let (ta, tb) = (
        find(&a.tables, "Parameter Estimates", 0),
        find(&b.tables, "Parameter Estimates", 0),
    );
    for k in 0..3 {
        close(num(&ta.rows[k][1]), num(&tb.rows[k][1]), 1e-9);
    }
    // a level whose doses are all equal cannot be fitted alone: no parallelism test, a warning
    let flat = SPRAY
        .replace("2 2 44 2", "2 8 44 9")
        .replace("2 4 40 5", "2 8 40 11")
        .replace("2 16 39 20", "2 8 39 10")
        .replace("2 32 42 33", "2 8 42 12");
    let r = run(
        &flat,
        "PROBIT down OF exposed BY strain(1,2) WITH dose /PRINT=PARALL.",
    );
    let tests = find(&r.tables, "Chi-Square Tests", 0);
    assert_eq!(tests.rows.len(), 1);
    assert!(
        r.warnings.iter().any(|w| w.contains("parallelism")),
        "{:?}",
        r.warnings
    );
    // a range below 0 keeps its levels apart
    let shifted = format!("{SPRAY}COMPUTE strain = strain - 2.\n");
    let r = run(
        &shifted,
        "PROBIT down OF exposed BY strain(-1,0) WITH dose /PRINT=NONE.",
    );
    let t = find(&r.tables, "Parameter Estimates", 0);
    close(
        num(&row(t, &["Intercept, strain = -1"])[1]),
        -2.00214601763,
        1e-6,
    );
    close(
        num(&row(t, &["Intercept, strain = 0"])[1]),
        -2.43111401164,
        1e-6,
    );
}

#[test]
fn curvefit_every_model_of_a_series() {
    let r = run(SERIES, "CURVEFIT y /MODEL=ALL /PLOT=NONE.");
    let t = find(&r.tables, "Model Summary and Parameter Estimates", 0);
    // R square, F, df2, constant, b1, b2, b3 (the multiplicative ones as exp)
    let e = |v: f64| v.exp();
    // b2 and b3 are NaN where the model has none
    let n = f64::NAN;
    let expected: [(&str, [f64; 7]); 11] = [
        (
            "Linear",
            [
                0.930858188456,
                134.6302863160,
                10.0,
                -1.50606060606,
                1.96118881119,
                n,
                n,
            ],
        ),
        (
            "Logarithmic",
            [
                0.709050237246,
                24.3701947214,
                10.0,
                -2.35808428986,
                8.16507030100,
                n,
                n,
            ],
        ),
        (
            "Inverse",
            [
                0.407797355166,
                6.8861116836,
                10.0,
                15.8387526727,
                -17.7767601987,
                n,
                n,
            ],
        ),
        (
            "Quadratic",
            [
                0.997104491603,
                1549.6312210688,
                9.0,
                3.688636363636,
                -0.265109890110,
                0.171253746254,
                n,
            ],
        ),
        (
            "Cubic",
            [
                0.999519017000,
                5541.5348300095,
                8.0,
                2.1737373737374,
                0.9057461057461,
                -0.0451603951604,
                0.0110981610982,
            ],
        ),
        (
            "Compound",
            [
                0.998868173040,
                8825.2728399112,
                10.0,
                e(0.973164564357),
                e(0.190780366372),
                n,
                n,
            ],
        ),
        (
            "Power",
            [
                0.906097700828,
                96.4936650987,
                10.0,
                e(0.769520472451),
                0.866783997523,
                n,
                n,
            ],
        ),
        (
            "S",
            [
                0.630321974689,
                17.0505664803,
                10.0,
                2.74995177668,
                -2.07545624153,
                n,
                n,
            ],
        ),
        (
            "Growth",
            [
                0.998868173040,
                8825.2728399112,
                10.0,
                0.973164564357,
                0.190780366372,
                n,
                n,
            ],
        ),
        (
            "Exponential",
            [
                0.998868173040,
                8825.2728399112,
                10.0,
                e(0.973164564357),
                0.190780366372,
                n,
                n,
            ],
        ),
        (
            "Logistic",
            [
                0.998868173040,
                8825.2728399112,
                10.0,
                e(-0.973164564357),
                e(-0.190780366372),
                n,
                n,
            ],
        ),
    ];
    assert_eq!(t.rows.len(), 11);
    for (name, [r2, f, df2, b0, b1, b2, b3]) in expected {
        let line = row(t, &[name]);
        close(num(&line[1]), r2, 1e-9);
        close(num(&line[2]), f, 1e-7);
        assert_eq!(num(&line[4]), df2);
        close(num(&line[6]), b0, 1e-8);
        close(num(&line[7]), b1, 1e-8);
        for (at, b) in [(8, b2), (9, b3)] {
            match b.is_nan() {
                true => assert_eq!(line[at], Cell::Empty),
                false => close(num(&line[at]), b, 1e-8),
            }
        }
    }
    assert!(t.caption.as_deref().unwrap().contains("case sequence"));
}

#[test]
fn curvefit_upper_bound_anova_and_saved_intervals() {
    let mut r = run(
        SERIES,
        "CURVEFIT VARIABLES=y /MODEL=LGSTIC LINEAR COMPOUND /UPPERBOUND=40 /PRINT=ANOVA /SAVE=PRED RESID CIN.",
    );
    let t = find(&r.tables, "Model Summary and Parameter Estimates", 0);
    let logistic = row(t, &["Logistic"]);
    close(num(&logistic[1]), 0.988007662875, 1e-9);
    close(num(&logistic[6]), (-0.844215264790f64).exp(), 1e-8);
    close(num(&logistic[7]), (-0.266972074341f64).exp(), 1e-8);
    assert_eq!(num(&logistic[10]), 40.0);
    // compound coefficients: b0 = exp(a) with standard error exp(a) se(a)
    let coefficients = find(&r.tables, "Coefficients", 1);
    assert!(
        coefficients
            .caption
            .as_deref()
            .unwrap()
            .starts_with("Compound")
    );
    let constant = row(coefficients, &["(Constant)"]);
    close(num(&constant[1]), 0.973164564357f64.exp(), 1e-8);
    close(
        num(&constant[2]),
        0.973164564357f64.exp() * 0.01494636773791,
        1e-7,
    );
    close(num(&constant[4]), 65.1104389656, 1e-7);
    let anova = find(&r.tables, "ANOVA", 0);
    assert_eq!(num(&anova.rows[1][2]), 10.0);
    // FIT_1 and its interval belong to the first model in order: LINEAR
    let fit = values(&r, "FIT_1");
    close(fit[2].unwrap(), 4.37750582751, 1e-9);
    close(values(&r, "LCL_1")[2].unwrap(), -0.491778279648, 1e-7);
    close(values(&r, "UCL_1")[11].unwrap(), 27.15294325518, 1e-7);
    close(values(&r, "ERR_1")[2].unwrap(), 4.6 - 4.37750582751, 1e-8);
    // COMPOUND: back from the log scale
    close(
        values(&r, "FIT_2")[2].unwrap(),
        1.54550566347f64.exp(),
        1e-8,
    );
    close(
        values(&r, "LCL_2")[11].unwrap(),
        3.20095554261f64.exp(),
        1e-7,
    );
    close(
        values(&r, "UCL_2")[11].unwrap(),
        3.32410237903f64.exp(),
        1e-7,
    );
    // LGSTIC: 1 / (exp(v) + 1/u), the limits swap
    close(
        values(&r, "FIT_3")[2].unwrap(),
        1.0 / ((-1.64513148781f64).exp() + 1.0 / 40.0),
        1e-8,
    );
    close(
        values(&r, "LCL_3")[2].unwrap(),
        1.0 / ((-1.37718091695f64).exp() + 1.0 / 40.0),
        1e-7,
    );
    close(
        values(&r, "UCL_3")[2].unwrap(),
        1.0 / ((-1.91308205867f64).exp() + 1.0 / 40.0),
        1e-7,
    );
    let label = run_label(&r, "LCL_2");
    assert!(
        label.contains("95% lower") && label.contains("Compound"),
        "{label}"
    );
    let charts = r.engine.take_charts();
    assert_eq!(charts.len(), 1);
    assert_eq!(charts[0].title, "CURVEFIT: y");
    // too small a bound is an error
    assert!(error(SERIES, "CURVEFIT y /MODEL=LGSTIC /UPPERBOUND=20.").contains("UPPERBOUND"));
}

fn run_label(run: &Run, name: &str) -> String {
    let data = run.engine.dataset().expect("data");
    data.variable(name)
        .unwrap()
        .label()
        .unwrap_or_default()
        .to_string()
}

#[test]
fn curvefit_without_constant_with_weights_and_undefined_forms() {
    let r = run(
        SERIES,
        "CURVEFIT y /MODEL=LINEAR COMPOUND /NOCONSTANT /SAVE=CIN.",
    );
    let t = find(&r.tables, "Model Summary and Parameter Estimates", 0);
    let linear = row(t, &["Linear"]);
    close(num(&linear[1]), 0.97777236008, 1e-9);
    close(num(&linear[2]), 483.879350193, 1e-8);
    assert_eq!(num(&linear[4]), 11.0);
    assert_eq!(linear[6], Cell::Empty);
    close(num(&linear[7]), 1.78046153846, 1e-9);
    let compound = row(t, &["Compound"]);
    close(num(&compound[1]), 0.960836990615, 1e-9);
    close(num(&compound[7]), 0.307560114095f64.exp(), 1e-8);
    close(values(&r, "LCL_1")[2].unwrap(), 0.76814587513, 1e-7);
    close(values(&r, "UCL_2")[11].unwrap(), 127.97483213187, 1e-7);
    // case weights count as cases against an independent variable
    let weighted = format!("{SERIES}WEIGHT BY w.\n");
    let r = run(
        &weighted,
        "CURVEFIT y WITH x /MODEL=QUADRATIC COMPOUND /PRINT=ANOVA.",
    );
    let t = find(&r.tables, "Model Summary and Parameter Estimates", 0);
    let quadratic = row(t, &["Quadratic"]);
    close(num(&quadratic[1]), 0.993532729421, 1e-9);
    close(num(&quadratic[2]), 1305.81024813, 1e-8);
    assert_eq!(num(&quadratic[4]), 17.0);
    close(num(&quadratic[8]), 0.342046265262, 1e-8);
    let coefficients = find(&r.tables, "Coefficients", 0);
    close(
        num(&row(coefficients, &["x ** 2"])[2]),
        0.0346913094418,
        1e-7,
    );
    let compound = row(t, &["Compound"]);
    close(num(&compound[1]), 0.990228407136, 1e-9);
    assert_eq!(num(&compound[4]), 18.0);
    // without an independent the weights do not count
    let r = run(&weighted, "CURVEFIT y /PLOT=NONE.");
    close(
        num(&find(&r.tables, "Model Summary and Parameter Estimates", 0).rows[0][2]),
        134.6302863160,
        1e-7,
    );
    // a zero makes the logarithm undefined: that model is left out with a warning
    let zero = SERIES.replace("3.1 1.5 1", "0 1.5 1");
    let r = run(&zero, "CURVEFIT y /MODEL=LINEAR GROWTH /PLOT=NONE.");
    assert_eq!(
        find(&r.tables, "Model Summary and Parameter Estimates", 0)
            .rows
            .len(),
        1
    );
    assert!(
        r.warnings.iter().any(|w| w.contains("Growth")),
        "{:?}",
        r.warnings
    );
}

#[test]
fn wls_power_search() {
    let r = run(
        SHOPS,
        "WLS cost WITH area staff /SOURCE=area /POWER=0 TO 2 BY 0.5 /SAVE=WEIGHT /PRINT=ALL.",
    );
    let ll = find(&r.tables, "Log-Likelihood Values", 0);
    let expected = [
        -42.5972205932,
        -41.6993005507,
        -40.8131543524,
        -40.0113000972,
        -39.3675249260,
    ];
    assert_eq!(ll.rows.len(), 5);
    for (line, value) in ll.rows.iter().zip(expected) {
        close(num(&line[1]), value, 1e-9);
    }
    assert!(ll.caption.as_deref().unwrap().contains("power 2 "));
    let summary = find(&r.tables, "Model Summary", 0);
    close(num(&row(summary, &["Multiple R"])[1]), 0.968182913097, 1e-9);
    close(
        num(&row(summary, &["Adjusted R Square"])[1]),
        0.926941178747,
        1e-9,
    );
    close(
        num(&row(summary, &["Std. Error of the Estimate"])[1]),
        1.11346346299,
        1e-9,
    );
    close(
        num(&row(summary, &["Log-likelihood Function Value"])[1]),
        -39.3675249260,
        1e-9,
    );
    let anova = find(&r.tables, "ANOVA", 0);
    close(num(&row(anova, &["Residual"])[1]), 14.87761060, 1e-8);
    close(num(&row(anova, &["Regression"])[4]), 89.8132074949, 1e-8);
    // five coefficient tables for PRINT=ALL, then the best one
    let best = find(&r.tables, "Coefficients", 5);
    let area = row(best, &["area"]);
    close(num(&area[1]), 6.24423029848, 1e-9);
    close(num(&area[2]), 1.56285609147, 1e-8);
    close(num(&area[3]), 0.774317783003, 1e-8);
    close(num(&area[6]), 0.00177635060277, 1e-6);
    close(num(&row(best, &["(Constant)"])[2]), 1.63826386099, 1e-8);
    let half = find(&r.tables, "Coefficients", 3);
    close(num(&row(half, &["staff"])[1]), 1.38222620821, 1e-9);
    let saved = values(&r, "WGT_1");
    close(saved[0].unwrap(), 1.2f64.powf(-2.0), 1e-12);
    assert!(r.warnings.is_empty(), "{:?}", r.warnings);
}

#[test]
fn wls_weight_variable_and_defaults() {
    let data = format!("{SHOPS}COMPUTE inv = 1 / staff.\n");
    let r = run(&data, "WLS cost WITH area staff /WEIGHT=inv /NOCONSTANT.");
    assert!(!r.tables.iter().any(|t| t.title == "Log-Likelihood Values"));
    let summary = find(&r.tables, "Model Summary", 0);
    close(num(&row(summary, &["R Square"])[1]), 0.985961170321, 1e-9);
    close(
        num(&row(summary, &["Log-likelihood Function Value"])[1]),
        -40.7832695715,
        1e-9,
    );
    let coefficients = find(&r.tables, "Coefficients", 0);
    close(num(&row(coefficients, &["staff"])[2]), 1.25994395921, 1e-8);
    assert_eq!(coefficients.rows.len(), 2);
    // no SOURCE, DELTA or WEIGHT: area to the power -1, with a warning
    let r = run(SHOPS, "WLS cost WITH area staff.");
    close(
        num(&find(&r.tables, "Log-Likelihood Values", 0).rows[0][1]),
        -40.8131543524,
        1e-9,
    );
    assert!(r.warnings.iter().any(|w| w.contains("neither")));
    // a power outside the range moves to its end
    let r = run(SHOPS, "WLS cost WITH area staff /DELTA=9.");
    assert_eq!(
        num(&find(&r.tables, "Log-Likelihood Values", 0).rows[0][0]),
        7.5
    );
}
