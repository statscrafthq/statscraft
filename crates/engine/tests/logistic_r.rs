//! LOGISTIC REGRESSION against R's glm on 120 invented cases (age, income, a three-level region
//! as a categorical covariate, the last level the reference; epsilon 1e-14): the coefficients,
//! standard errors, Wald statistics and Exp(B) with their intervals, the omnibus chi-square, the
//! -2 log likelihood with Cox and Snell and Nagelkerke R square, the classification table, the
//! Hosmer and Lemeshow statistic (ten groups of twelve by predicted probability), the score
//! statistics of block 0 (score test of each variable added to the constant-only model, written
//! out in R) and the saved casewise statistics in the manual's definitions (deviance residual,
//! studentized residual = deviance residual / sqrt(1 - h), logit and standardized residual,
//! leverage, Cook's distance = z^2 h / (1 - h) and DFBETA = (X'VX)^-1 x e / (1 - h)).

use statscraft_data::Values;
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /y age inc reg.
BEGIN DATA
0 54 27.1 1 
1 46 64.5 1 
0 41 41.7 1 
1 39 65.9 3 
0 31 57.3 2 
1 58 72 1 
0 36 59.4 3 
0 52 39.7 2 
0 42 73.4 1 
0 65 35.6 1 
0 43 34.8 3 
0 34 45.4 1 
1 38 42.5 2 
1 45 54 1 
0 28 54.8 1 
1 33 64.4 3 
0 47 57.1 3 
1 60 57.8 3 
0 59 53.2 3 
0 36 56.9 3 
1 64 50 3 
1 41 47.3 2 
0 32 42.4 1 
0 53 32.6 3 
0 56 37.2 1 
0 31 41.1 1 
1 47 25.4 1 
0 45 42.4 3 
1 52 50.8 2 
1 45 32.7 2 
0 61 48.9 1 
1 23 43.4 2 
0 59 37.9 1 
1 53 62.9 2 
1 50 53.8 3 
1 58 64.3 3 
1 47 51.2 1 
1 56 85.2 2 
1 56 59.7 3 
0 29 58.6 3 
0 28 8.4 1 
1 56 63 1 
1 28 21.8 2 
1 53 25.9 3 
1 38 55.1 3 
1 45 52 2 
0 29 24.9 2 
0 43 58.5 1 
1 48 52.9 2 
1 52 63.3 3 
0 42 58.1 2 
0 42 51 1 
1 51 47.3 2 
1 51 67.2 2 
1 61 69.2 1 
1 47 50.2 1 
0 43 25 1 
1 55 31.8 3 
0 56 57.4 3 
1 53 39.9 3 
1 58 64.1 1 
0 47 42.3 1 
0 49 43.5 1 
1 49 45.9 1 
0 64 60.1 3 
0 48 56.7 2 
1 70 50.6 3 
0 33 69.4 3 
1 60 70.9 2 
1 46 62.4 1 
1 52 19.4 2 
0 43 45.2 3 
0 19 35.8 2 
1 58 35.7 1 
1 52 29.4 1 
0 22 33.5 1 
0 58 54.3 3 
1 37 51.9 2 
1 71 53.3 3 
0 39 50 1 
1 40 54.8 2 
1 49 51.6 3 
0 44 32.5 3 
0 45 40.8 1 
1 43 80 1 
1 57 48.7 2 
1 60 53.7 3 
1 33 50.1 2 
1 53 50.3 2 
1 35 48.9 1 
1 61 51.6 2 
0 39 9.7 1 
0 50 42.7 1 
0 58 39.4 1 
1 45 55.4 3 
0 57 43.1 3 
0 26 59.6 3 
0 40 25.5 1 
0 38 31.7 3 
0 70 47.2 3 
1 47 35.2 1 
1 49 25.2 2 
1 50 37.5 2 
1 53 50.7 2 
1 46 66.6 3 
1 57 27.7 2 
1 33 72.5 2 
0 43 44.2 3 
0 27 45.5 3 
0 38 30.5 2 
1 55 34.6 2 
1 35 71.4 1 
1 48 36.2 3 
0 43 28.9 3 
1 36 42.9 2 
1 57 36.5 2 
0 32 67 3 
0 54 40.5 3 
1 30 66.8 1 
0 66 28.6 3 
END DATA.
";

fn run() -> (Engine, Vec<Table>) {
    let source = format!(
        "{DATA}LOGISTIC REGRESSION VARIABLES=y WITH age inc reg /CATEGORICAL=reg \
         /METHOD=ENTER age inc reg /PRINT=GOODFIT CI(95) \
         /SAVE=PRED(p) PGROUP(pg) RESID(rs) DEV(dv) LRESID(lr) SRESID(sr) ZRESID(zr) LEVER(lv) COOK(ck) DFBETA(db)."
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

fn near(got: f64, want: f64, tolerance: f64, what: &str) {
    assert!(
        (got - want).abs() <= tolerance * want.abs().max(1e-3),
        "{what}: {got} vs {want}"
    );
}

fn table<'a>(tables: &'a [Table], title: &str) -> &'a Table {
    tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"))
}

/// The number in `column` of the row whose cells include `label`.
fn cell(t: &Table, label: &str, column: &str) -> f64 {
    let at = t
        .column_titles()
        .iter()
        .position(|c| c == column || c.ends_with(&format!("/ {column}")))
        .unwrap_or_else(|| panic!("no column {column} in {:?}", t.column_titles()));
    let row = t
        .rows
        .iter()
        .find(|r| r.iter().any(|c| matches!(c, Cell::Text(s) if s == label)))
        .unwrap_or_else(|| panic!("no row {label} in {}", t.title));
    match row[at] {
        Cell::Number(n) => n,
        ref other => panic!("{label} {column}: {other:?}"),
    }
}

#[test]
fn the_equation_and_the_fit_agree_with_r() {
    let (_, t) = run();
    // the last table of that name is the fitted model's, the first is block 0's
    let eq = t
        .iter()
        .rev()
        .find(|t| t.title == "Variables in the Equation")
        .expect("the equation");
    // the last block's table: the constant and the four coded variables
    for (label, b, se, wald, lower, upper) in [
        (
            "age",
            0.0596325291149,
            0.0200060472433,
            8.884722701307,
            1.02063134,
            1.103893705,
        ),
        (
            "inc",
            0.0507220724222,
            0.0166068747523,
            9.328636251255,
            1.018339333,
            1.086836246,
        ),
        (
            "reg(1)",
            0.2906710751222,
            0.4842025349316,
            0.360370735746,
            0.5177056432,
            3.45454448,
        ),
        (
            "reg(2)",
            2.3616793673102,
            0.6217507546554,
            14.428102823221,
            3.136383175,
            35.8838902,
        ),
    ] {
        // two steps' tables carry these labels; the first row found is block 0's, which has none
        let rows: Vec<&Vec<Cell>> = eq
            .rows
            .iter()
            .filter(|r| r.iter().any(|c| matches!(c, Cell::Text(s) if s == label)))
            .collect();
        let row = rows.last().expect("a row");
        let at = |name: &str| {
            let c = eq
                .column_titles()
                .iter()
                .position(|x| x == name || x.ends_with(&format!("/ {name}")))
                .unwrap_or_else(|| panic!("no column {name} in {:?}", eq.column_titles()));
            match row[c] {
                Cell::Number(n) => n,
                ref o => panic!("{label} {name}: {o:?}"),
            }
        };
        near(at("B"), b, 1e-7, label);
        near(at("S.E."), se, 1e-7, label);
        near(at("Wald"), wald, 1e-7, label);
        near(at("Exp(B)"), b.exp(), 1e-7, label);
        near(at("Lower"), lower, 1e-7, label);
        near(at("Upper"), upper, 1e-7, label);
    }
    let omnibus = table(&t, "Omnibus Tests of Model Coefficients");
    near(
        cell(omnibus, "Model", "Chi-square"),
        33.95753769,
        1e-8,
        "omnibus",
    );
    let summary = table(&t, "Model Summary");
    near(
        cell(summary, "Step 1", "-2 Log likelihood"),
        131.5634851,
        1e-8,
        "-2LL",
    );
    near(
        cell(summary, "Step 1", "Cox & Snell R Square"),
        0.2464647503,
        1e-8,
        "Cox and Snell",
    );
    near(
        cell(summary, "Step 1", "Nagelkerke R Square"),
        0.3293856787,
        1e-8,
        "Nagelkerke",
    );
    let hl = table(&t, "Hosmer and Lemeshow Test");
    near(
        cell(hl, "Step 1", "Chi-square"),
        15.76571999,
        1e-8,
        "Hosmer and Lemeshow",
    );
    near(cell(hl, "Step 1", "df"), 8.0, 1e-12, "df");
    near(
        cell(hl, "Step 1", "Sig."),
        0.04585879788,
        1e-8,
        "significance",
    );
    let classification = table(&t, "Classification Table");
    near(
        cell(classification, "Overall Percentage", "Percentage Correct"),
        54.16666667,
        1e-8,
        "block 0 overall",
    );
}

#[test]
fn the_score_statistics_of_block_zero_agree_with_r() {
    let (_, t) = run();
    let scores = table(&t, "Variables not in the Equation");
    // the block 0 estimate of the constant stops at BCON 0.001, so these agree to about 1e-6
    for (label, score) in [
        ("age", 6.092851285),
        ("inc", 7.447417272),
        ("reg", 13.29754896),
        ("reg(1)", 3.328978714),
        ("reg(2)", 13.28243521),
        ("Overall Statistics", 29.13457661),
    ] {
        near(cell(scores, label, "Score"), score, 1e-5, label);
    }
}

fn saved(engine: &Engine, name: &str) -> Vec<f64> {
    let data = engine.dataset().unwrap();
    let Values::Numeric(v) = data.variable(name).unwrap().values() else {
        panic!("{name} is not numeric")
    };
    (0..data.case_count()).map(|c| v.get(c).unwrap()).collect()
}

#[test]
fn the_saved_casewise_statistics_follow_the_manuals_definitions() {
    let (engine, _) = run();
    let cases = [0usize, 6, 49, 119];
    let expected: [(&str, [f64; 4]); 14] = [
        ("p", [0.3022835502, 0.3630429773, 0.6433090636, 0.416916383]),
        ("pg", [0.0, 0.0, 1.0, 0.0]),
        (
            "rs",
            [-0.3022835502, -0.3630429773, 0.3566909364, -0.416916383],
        ),
        (
            "dv",
            [-0.8484603599, -0.9497927079, 0.9392869757, -1.038676733],
        ),
        (
            "lr",
            [-1.433246988, -1.569964636, 1.554462787, -1.715019889],
        ),
        (
            "sr",
            [-0.8712152278, -0.9710610815, 0.9568544881, -1.081667414],
        ),
        (
            "zr",
            [-0.6582150012, -0.7549600225, 0.7446225803, -0.8455884867],
        ),
        (
            "lv",
            [0.05155489767, 0.04332469283, 0.03638222097, 0.07790999667],
        ),
        (
            "ck",
            [0.02355012859, 0.02581183247, 0.0209342211, 0.06041405606],
        ),
        (
            "db0",
            [
                -0.05747357052,
                -0.1007001586,
                -0.05867228557,
                -0.04825545959,
            ],
        ),
        (
            "db1",
            [
                -0.0006012027942,
                0.001835221873,
                0.0006575733875,
                -0.002497192142,
            ],
        ),
        (
            "db2",
            [
                0.001708005647,
                -0.0006139122498,
                0.001313879205,
                0.002383330486,
            ],
        ),
        (
            "db3",
            [-0.03425584198, 0.04483316143, -0.03495164329, 0.04998942343],
        ),
        (
            "db4",
            [0.01252630374, 0.0502768314, -0.0219597606, 0.05372529323],
        ),
    ];
    for (name, want) in expected {
        let got = saved(&engine, name);
        assert_eq!(got.len(), 120, "{name}");
        for (case, want) in cases.iter().zip(want) {
            near(got[*case], want, 1e-7, &format!("{name}[{case}]"));
        }
    }
}
