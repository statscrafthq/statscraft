//! REGRESSION options against R's lm on 60 invented cases (four predictors): the descriptive
//! tables (means, standard deviations, correlations with one-tailed significance, cross-products,
//! covariances), METHOD=TEST subset tests (`anova` of the reduced fits), METHOD=REMOVE, SELECT
//! (lm on the selected cases, SAVE for all of them), the F criteria FIN and FOUT, MAXSTEPS, the
//! probability criteria, CI(90) and the data of the partial regression, scatter and normal P-P
//! plots. The stepwise paths are written out in R with the F-to-enter and F-to-remove of the
//! manual (the partial F of each candidate). Scripts in lab/oracles/regression-r3.

use statscraft_data::Values;
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /y x1 x2 x3 x4 sel.
BEGIN DATA.
8.35 11.2 10.3 0.6 6.1 1
7.22 8.2 5.1 4.3 7.7 1
11.84 7 1.1 5.8 7 1
8.74 9.5 6.8 6.1 4.4 0
15.37 15.1 8.5 9.4 2.6 1
9.61 6.5 3.9 6.1 1.4 1
6.39 11.4 10.3 6.7 3.7 1
10.47 11 8.5 5.8 5.3 0
8.92 8 4.8 3.7 7.2 1
9.08 10.9 7.5 8.6 0.1 1
14.2 13.1 7.2 6.5 5.8 1
11.67 2.7 -3.3 7.3 3.2 0
7.85 6.3 5 7.6 8 1
13.21 11.5 6.6 4.4 4.7 1
14.73 15.3 7.8 7.7 5.8 1
12.98 11.2 11.3 5.1 4 0
9.31 9.1 5.3 7.1 2.1 1
11.64 6.5 4.1 6.9 5.5 1
8.45 9.1 4.2 1.2 0.1 1
15.85 15.3 7.1 1.7 0.6 0
18.26 10.1 3.6 7.4 3.6 1
15.4 11.6 2.6 5.8 4.8 1
12.38 11.7 8.2 5.3 3 1
13.96 7.5 -2.5 7.4 2.2 0
12.17 7.3 8.5 6 7.8 1
9.79 6.3 4.5 3 5.8 1
10.54 10.3 4.8 4.4 5.7 1
9.09 11.7 14.7 4.5 7.2 0
9.54 7.5 4.2 7.8 0.1 1
8.87 11.3 7.1 7.8 7.1 1
18.37 12.5 1 4.1 7.7 1
7.41 4.9 5.6 5 2.7 0
18.41 9.7 3 2.7 1.4 1
14.8 11.4 2.5 6.7 4.3 1
13.87 10.9 6.1 3.1 6 1
6.4 11.5 10.4 1.1 1.7 0
16.13 10.4 2.4 6.4 5 1
16.03 11.2 7.5 7.6 3.9 1
14.24 11.4 8.3 6.6 7.2 1
6.18 4.8 0.4 2.7 1.3 0
8.99 9.1 4.3 7.9 0.7 1
11.74 13.7 8.1 4.4 4.1 1
8.41 8.3 5.7 4.3 1.4 1
11.3 9.8 3.1 6.3 7 0
15.71 12.7 1 4 1.5 1
12.02 7.1 2.8 5.8 6.4 1
12.57 12.6 8.3 7.3 3.5 1
12.68 15.7 9.3 2.9 5.5 0
12.32 12.9 5.9 3.8 6.5 1
12.5 14.4 8.9 6.1 3.7 1
13.41 12.5 3.2 5.5 4.8 1
14.27 10 2.7 8.7 5.4 0
16.17 12 5.8 7.7 3.8 1
7.66 5.8 5.4 6.3 2.4 1
19.22 13.3 6.4 9.8 6.9 1
9.18 10.2 7.8 1.7 6.3 0
17.5 15.7 8.1 8.6 7.3 1
11.6 6.1 1.8 6.4 6.3 1
5.16 5.4 5.2 5.9 0.4 1
7.98 6.5 1.5 5.2 2 0
END DATA.
";

fn engine_run(syntax: &str) -> (Engine, Vec<Table>) {
    let mut engine = Engine::default();
    let (commands, errors) = parse(&format!("{DATA}{syntax}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut tables = Vec::new();
    for command in &commands {
        match engine.execute(&command.command) {
            Ok(more) => tables.extend(more),
            Err(error) => panic!("line {}: {error}", command.first_line),
        }
    }
    (engine, tables)
}

fn run(syntax: &str) -> Vec<Table> {
    engine_run(syntax).1
}

/// The row headers joined with " / ", an empty header taking the one above it.
fn row_keys(table: &Table) -> Vec<String> {
    let mut carried = vec![String::new(); table.row_headers];
    table
        .rows
        .iter()
        .map(|row| {
            for (level, cell) in row.iter().take(table.row_headers).enumerate() {
                let shown = match cell {
                    Cell::Text(t) => Some(t.clone()),
                    Cell::Number(n) => Some(format!("{n}")),
                    _ => None,
                };
                if let Some(t) = shown {
                    carried[level] = t;
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

/// A cell of the `nth` table with this title.
fn cell<'t>(tables: &'t [Table], title: &str, nth: usize, row: &str, column: &str) -> &'t Cell {
    let table = tables
        .iter()
        .filter(|t| t.title == title)
        .nth(nth)
        .unwrap_or_else(|| panic!("no table {title} number {nth}"));
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
    match cell(tables, title, 0, row, column) {
        Cell::Number(n) => *n,
        other => panic!("{title} / {row} / {column}: {other:?}"),
    }
}

fn near(actual: f64, expected: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= 1e-8 + 1e-7 * expected.abs(),
        "{what}: {actual} is not {expected}"
    );
}

fn values(engine: &Engine, name: &str) -> Vec<Option<f64>> {
    let data = engine.dataset().unwrap();
    match data.variable(name).unwrap().values() {
        Values::Numeric(v) => (0..data.case_count()).map(|k| v.get(k)).collect(),
        Values::String(_) => panic!("{name} is a string"),
    }
}

const VARS: [&str; 5] = ["y", "x1", "x2", "x3", "x4"];
/// R's lower triangle by column: (x1,y) (x2,y) (x3,y) (x4,y) (x2,x1) (x3,x1) (x4,x1) (x3,x2) (x4,x2) (x4,x3).
const PAIRS: [(usize, usize); 10] = [
    (1, 0),
    (2, 0),
    (3, 0),
    (4, 0),
    (2, 1),
    (3, 1),
    (4, 1),
    (3, 2),
    (4, 2),
    (4, 3),
];

#[test]
fn descriptive_tables_agree_with_r() {
    let t = run("REGRESSION /DESCRIPTIVES=ALL /DEPENDENT=y /METHOD=ENTER x1 x2 x3 x4.");
    let (mean, sd) = (
        [11.80183333, 10.11166667, 5.505, 5.61, 4.328333333],
        [3.488788577, 3.028681632, 3.295473269, 2.13491555, 2.3407294],
    );
    for (i, v) in VARS.iter().enumerate() {
        near(value(&t, "Descriptive Statistics", v, "Mean"), mean[i], v);
        near(
            value(&t, "Descriptive Statistics", v, "Std. Deviation"),
            sd[i],
            v,
        );
        near(
            value(&t, "Descriptive Statistics", v, "Variance"),
            sd[i] * sd[i],
            v,
        );
        assert_eq!(value(&t, "Descriptive Statistics", v, "N"), 60.0);
    }
    let (cor, sig, cov, xprod) = (
        [
            0.5383995695,
            -0.1433767951,
            0.2748924462,
            0.2081202275,
            0.5349459123,
            0.01633844765,
            0.1578170728,
            -0.1122699086,
            0.1587764024,
            0.06014482864,
        ],
        [
            4.55713381e-06,
            0.1372215421,
            0.01676606613,
            0.05527705223,
            5.330747068e-06,
            0.4506960243,
            0.1142378708,
            0.1965386647,
            0.1128118428,
            0.3240176694,
        ],
        [
            5.688961299,
            -1.648433051,
            2.047472881,
            1.699574294,
            5.339262712,
            0.1056440678,
            1.118816384,
            -0.7898813559,
            1.224771186,
            0.300559322,
        ],
        [
            335.6487167,
            -97.25755,
            120.8009,
            100.2748833,
            315.0165,
            6.233,
            66.01016667,
            -46.603,
            72.2615,
            17.733,
        ],
    );
    for (k, (a, b)) in PAIRS.iter().enumerate() {
        for (row, col) in [(VARS[*a], VARS[*b]), (VARS[*b], VARS[*a])] {
            let at = |kind: &str| value(&t, "Correlations", &format!("{row} / {kind}"), col);
            near(at("Pearson Correlation"), cor[k], row);
            near(at("Sig. (1-tailed)"), sig[k], row);
            near(at("Covariance"), cov[k], row);
            near(at("Sum of Squares and Cross-products"), xprod[k], row);
            assert_eq!(at("N"), 60.0);
        }
    }
}

#[test]
fn subset_tests_remove_and_the_selected_cases_agree_with_r() {
    let t = run(
        "REGRESSION /STATISTICS=COEFF R ANOVA CHANGE /DEPENDENT=y /METHOD=ENTER x1 x2 /METHOD=TEST(x3)(x4)(x3 x4).",
    );
    for (label, want) in [
        (
            "Set 1: x3",
            [
                0.03354672069,
                24.09080919,
                1.0,
                24.09080919,
                4.771695424,
                0.0332118143,
            ],
        ),
        (
            "Set 2: x4",
            [
                0.02472704169,
                17.7571587,
                1.0,
                17.7571587,
                3.517181688,
                0.06604788643,
            ],
        ),
        (
            "Set 3: x3, x4",
            [
                0.06275299737,
                45.06462791,
                2.0,
                22.53231396,
                4.463002408,
                0.01598972424,
            ],
        ),
    ] {
        let at = |c: &str| value(&t, "Subset Tests", &format!("2 / {label}"), c);
        near(at("R Square Change"), want[0], label);
        near(at("Sum of Squares"), want[1], label);
        assert_eq!(at("df"), want[2]);
        near(at("Mean Square"), want[3], label);
        near(at("F"), want[4], label);
        near(at("Sig."), want[5], label);
    }
    let r = [277.6779294, 55.0, 5.048689626];
    near(
        value(&t, "Subset Tests", "2 / Residual", "Sum of Squares"),
        r[0],
        "residual",
    );
    assert_eq!(value(&t, "Subset Tests", "2 / Residual", "df"), r[1]);
    near(
        value(&t, "Subset Tests", "2 / Residual", "Mean Square"),
        r[2],
        "residual",
    );

    // REMOVE takes x2 out of the equation of all four
    let t = run("REGRESSION /DEPENDENT=y /METHOD=ENTER x1 x2 x3 x4 /METHOD=REMOVE x2.");
    let (coef, se, fit) = (
        [2.686693376, 0.5951304491, 0.4245175107, 0.1653837499],
        [1.658434659, 0.1234707602, 0.1732794932, 0.160027822],
        [0.3726648189, 0.3390575771, 2.836328088],
    );
    for (i, row) in ["(Constant)", "x1", "x3", "x4"].iter().enumerate() {
        near(
            value(
                &t,
                "Coefficients",
                &format!("2 / {row}"),
                "Unstandardized Coefficients / B",
            ),
            coef[i],
            row,
        );
        near(
            value(
                &t,
                "Coefficients",
                &format!("2 / {row}"),
                "Unstandardized Coefficients / Std. Error",
            ),
            se[i],
            row,
        );
    }
    near(
        value(&t, "Model Summary", "2", "R Square"),
        fit[0],
        "R square",
    );
    near(
        value(&t, "Model Summary", "2", "Adjusted R Square"),
        fit[1],
        "adjusted",
    );
    near(
        value(&t, "Model Summary", "2", "Std. Error of the Estimate"),
        fit[2],
        "se",
    );
    assert_eq!(
        cell(&t, "Variables Entered/Removed", 0, "2", "Variables Removed"),
        &Cell::Text("x2".into())
    );

    // SELECT: the equation of the 45 cases with sel = 1, predictions for every case
    let (engine, t) = engine_run(
        "REGRESSION /SELECT=sel EQ 1 /STATISTICS=COEFF R ANOVA /DEPENDENT=y /METHOD=ENTER x1 x2 x3 x4 /SAVE=PRED RESID.",
    );
    let (coef, se, fit, f) = (
        [
            3.648368722,
            0.9631588803,
            -0.7697721298,
            0.2915231785,
            0.2551337368,
        ],
        [
            1.686030944,
            0.1415930686,
            0.1590349582,
            0.1794200862,
            0.1481132585,
        ],
        [0.777779467, 0.6049408993, 0.5654349892, 2.353019422],
        [15.31266836, 1.12324249e-07],
    );
    for (i, row) in ["(Constant)", "x1", "x2", "x3", "x4"].iter().enumerate() {
        near(
            value(
                &t,
                "Coefficients",
                &format!("1 / {row}"),
                "Unstandardized Coefficients / B",
            ),
            coef[i],
            row,
        );
        near(
            value(
                &t,
                "Coefficients",
                &format!("1 / {row}"),
                "Unstandardized Coefficients / Std. Error",
            ),
            se[i],
            row,
        );
    }
    for (c, w) in [
        "R",
        "R Square",
        "Adjusted R Square",
        "Std. Error of the Estimate",
    ]
    .iter()
    .zip(fit)
    {
        near(value(&t, "Model Summary", "1", c), w, c);
    }
    near(value(&t, "ANOVA", "1 / Regression", "F"), f[0], "F");
    near(value(&t, "ANOVA", "1 / Regression", "Sig."), f[1], "F sig");
    let (pred, resid) = (values(&engine, "PRE_1"), values(&engine, "RES_1"));
    for (k, (p, r)) in [0usize, 3, 4, 7].iter().zip(
        [8.238324946, 10.46480743, 15.0526703, 10.74309654]
            .iter()
            .zip([0.1116750541, -1.724807433, 0.3173296951, -0.2730965423]),
    ) {
        near(pred[*k].unwrap(), *p, "PRED");
        near(resid[*k].unwrap(), r, "RESID");
    }
}

#[test]
fn stepwise_with_f_criteria_maxsteps_and_probabilities_agree_with_r() {
    // (criteria, entered in order, R square at each step)
    let paths: [(&str, &[&str], f64); 4] = [
        ("FIN(4) FOUT(3)", &["x1", "x2", "x3"], 0.5886033422),
        ("FIN(2) FOUT(1.5)", &["x1", "x2", "x3", "x4"], 0.6133303839),
        ("FIN(7) FOUT(3)", &["x1", "x2"], 0.5505773865),
        (
            "MAXSTEPS(2) PIN(0.9) POUT(0.95)",
            &["x1", "x2"],
            0.5505773865,
        ),
    ];
    for (criteria, entered, final_r2) in paths {
        let t = run(&format!(
            "REGRESSION /CRITERIA={criteria} /DEPENDENT=y /METHOD=STEPWISE x1 x2 x3 x4."
        ));
        let table = t
            .iter()
            .find(|t| t.title == "Variables Entered/Removed")
            .expect("entered");
        assert_eq!(table.rows.len(), entered.len(), "{criteria}");
        for (i, name) in entered.iter().enumerate() {
            assert_eq!(
                table.rows[i][1],
                Cell::Text((*name).into()),
                "{criteria} step {i}"
            );
        }
        near(
            value(
                &t,
                "Model Summary",
                &format!("{}", entered.len()),
                "R Square",
            ),
            final_r2,
            criteria,
        );
    }
}

#[test]
fn confidence_limits_and_the_plot_data_agree_with_r() {
    let (mut engine, t) = engine_run(
        "REGRESSION /STATISTICS=COEFF CI(90) /DEPENDENT=y /METHOD=ENTER x1 x2 x3 x4 /PARTIALPLOT=x3 /RESIDUALS=NORMPROB(ZRESID).",
    );
    let (lower, upper) = (
        [
            0.6888557525,
            0.7581759351,
            -0.8026287747,
            0.07099440877,
            0.02578232971,
        ],
        [
            5.086441799,
            1.14354025,
            -0.4456783464,
            0.5355194444,
            0.4520544678,
        ],
    );
    for (i, row) in ["(Constant)", "x1", "x2", "x3", "x4"].iter().enumerate() {
        near(
            value(
                &t,
                "Coefficients",
                &format!("1 / {row}"),
                "90.0% Confidence Interval for B / Lower Bound",
            ),
            lower[i],
            row,
        );
        near(
            value(
                &t,
                "Coefficients",
                &format!("1 / {row}"),
                "90.0% Confidence Interval for B / Upper Bound",
            ),
            upper[i],
            row,
        );
    }
    let charts: Vec<serde_json::Value> = engine
        .take_charts()
        .iter()
        .map(|c| serde_json::to_value(c).expect("chart"))
        .collect();
    let number = |v: &serde_json::Value| v.as_f64().expect("a number");
    // the residuals of y and of x3 on the other three predictors
    let partial = charts
        .iter()
        .find(|c| c["kind"] == "scatter")
        .expect("partial plot");
    let points = partial["plot"]["groups"][0]["points"]
        .as_array()
        .expect("points");
    assert_eq!(points.len(), 60);
    let (px, py) = (
        [-4.653572787, -1.445295611, -0.2735845634],
        [-1.80906231, -3.863474337, -0.5313718875],
    );
    for i in 0..3 {
        near(number(&points[i]["x"]), px[i], "partial x");
        near(number(&points[i]["y"]), py[i], "partial y");
    }
    let sums = [9.658940314e-15, -2.831068713e-15, 79.4402603, 261.9569524];
    let (sx, sy, sxy, sxx) = points.iter().fold((0.0, 0.0, 0.0, 0.0), |a, p| {
        let (x, y) = (number(&p["x"]), number(&p["y"]));
        (a.0 + x, a.1 + y, a.2 + x * y, a.3 + x * x)
    });
    assert!(sx.abs() < 1e-9 && sy.abs() < 1e-9);
    near(sxy, sums[2], "cross-product");
    near(sxx, sums[3], "sum of squares of x");
    // the slope of the partial plot is the coefficient of x3
    near(sxy / sxx, 0.3032569266, "partial slope");
    // the normal P-P plot: i / n against the normal cdf of the sorted residuals over s
    let pp = charts
        .iter()
        .find(|c| c["kind"] == "normal_pp")
        .expect("P-P plot");
    let points = pp["plot"]["groups"][0]["points"]
        .as_array()
        .expect("points");
    let first = [0.02529590754, 0.04436951701, 0.06370663403];
    for (i, want) in first.iter().enumerate() {
        near(number(&points[i]["x"]), (i as f64 + 1.0) / 60.0, "observed");
        near(number(&points[i]["y"]), *want, "expected");
    }
    near(
        points.iter().map(|p| number(&p["y"])).sum::<f64>(),
        29.04983817,
        "sum of the expected",
    );
}
