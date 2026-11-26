//! CATPCA on synthetic data. The expected numbers come from independent computations: a
//! general statistics package's optimal scaling principal components (numeric, ordinal and
//! nominal levels; its eigenvalues of the quantified variables and loadings), plain principal
//! components of the correlation matrix, the exact eigen solution when numeric and multiple
//! nominal variables mix, and a separate script that runs alternating least squares on the
//! loss for the spline levels, passive missing values, case weights and variable weights.
//! Loadings follow our reflection rule.

use statscraft_data::Values;
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

/// Twenty restaurant visits rated 1 to 5 on taste, price, service and noise, and the area of
/// town (1 center, 2 harbor, 3 hills).
const DATA: &str = "DATA LIST FREE / taste price service noise area.
BEGIN DATA
3 2 4 1 1
4 3 4 2 2
2 1 2 1 1
5 4 5 3 3
1 1 1 2 1
3 3 3 3 2
4 4 5 4 3
2 2 1 1 1
5 5 4 4 3
3 2 3 2 2
1 2 2 1 1
4 3 5 3 3
2 3 2 4 2
5 4 4 2 3
3 1 3 1 1
4 5 3 4 2
1 1 2 2 1
2 2 3 3 2
5 5 5 4 3
3 4 2 3 2
END DATA.
VALUE LABELS area 1 'center' 2 'harbor' 3 'hills'.
";

/// The same visits with three passive gaps (0 is missing) and a case weight.
const GAPS: &str = "DATA LIST FREE / taste price service noise wt.
BEGIN DATA
3 2 4 1 1
4 3 4 2 2
2 0 2 1 1
5 4 5 3 1
1 1 1 2 3
3 3 3 3 1
4 4 5 0 1
2 2 1 1 2
5 5 4 4 1
3 2 3 2 1
1 2 2 1 1
4 3 5 3 2
2 3 2 4 1
0 4 4 2 1
3 1 3 1 1
4 5 3 4 1
1 1 2 2 2
2 2 3 3 1
5 5 5 4 1
3 4 2 3 1
END DATA.
WEIGHT BY wt.
";

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

fn table<'t>(tables: &'t [Table], title: &str) -> &'t Table {
    tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title} in {:?}", titles(tables)))
}

fn titles(tables: &[Table]) -> Vec<&str> {
    tables.iter().map(|t| t.title.as_str()).collect()
}

fn value(tables: &[Table], title: &str, row: &str, column: &str) -> f64 {
    let t = table(tables, title);
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

const EIGEN: &str = "Variance Accounted For / Total (Eigenvalue)";
const D1: &str = "Dimension 1";
const D2: &str = "Dimension 2";
const VARS: [&str; 4] = ["taste", "price", "service", "noise"];

/// Checks the model summary eigenvalues and the loadings of the four ratings.
fn check_solution(t: &[Table], eigen: [f64; 2], loadings: [[f64; 2]; 4], tolerance: f64) {
    check(t, "Model Summary", "1", EIGEN, eigen[0], tolerance);
    check(t, "Model Summary", "2", EIGEN, eigen[1], tolerance);
    for (name, l) in VARS.iter().zip(loadings) {
        check(t, "Component Loadings", name, D1, l[0], tolerance);
        check(t, "Component Loadings", name, D2, l[1], tolerance);
    }
}

#[test]
fn numeric_levels_give_principal_components() {
    let t = tables(
        "CATPCA taste price service noise /ANALYSIS=taste price service noise (LEVEL=NUME)
         /PRINT=LOADING CORR OCORR HISTORY.",
    );
    // the eigenvalues of the correlation matrix and its principal component loadings
    check_solution(
        &t,
        [2.99440212, 0.70173944],
        [
            [0.92087884, -0.30360053],
            [0.92349500, 0.23523297],
            [0.83289580, -0.46543562],
            [0.77448425, 0.58103467],
        ],
        1e-6,
    );
    check(
        t.as_slice(),
        "Model Summary",
        "Total",
        EIGEN,
        3.69614156,
        1e-6,
    );
    // alpha of eigenvalue e with four variables: 4(e - 1)/(3e)
    let e = 2.99440212;
    check(
        &t,
        "Model Summary",
        "1",
        "Cronbach's Alpha",
        4.0 * (e - 1.0) / (3.0 * e),
        1e-6,
    );
    check(
        &t,
        "Model Summary",
        "1",
        "Variance Accounted For / % of Variance",
        25.0 * e,
        1e-5,
    );
    // numeric quantifications keep the correlations of the codes
    for (a, b) in [("taste", "price"), ("service", "noise")] {
        let original = value(&t, "Correlations Original Variables", a, b);
        check(
            &t,
            "Correlations Transformed Variables",
            a,
            b,
            original,
            1e-9,
        );
    }
    // all numeric: no first phase to show, and the first step already converges
    let history = table(&t, "Iteration History");
    assert_eq!(history.rows.len(), 1);
    assert_eq!(history.rows[0][0], Cell::Number(1.0));
}

#[test]
fn ordinal_and_nominal_levels_match_optimal_scaling() {
    let t = tables(
        "CATPCA taste price service noise /ANALYSIS=taste price service noise (LEVEL=ORDI)
         /CRITITER=1e-12 /MAXITER=5000 /PRINT=LOADING QUANT(service).",
    );
    check_solution(
        &t,
        [2.96744259, 0.83616127],
        [
            [0.94026361, -0.28301810],
            [0.93099387, 0.27404222],
            [0.79563986, -0.56429447],
            [0.76390744, 0.60210849],
        ],
        1e-5,
    );
    // ordinal quantifications never fall and have weighted sum of squares n
    let q = table(&t, "Quantifications: service");
    let values: Vec<f64> = q
        .rows
        .iter()
        .map(|r| match (&r[1], &r[2]) {
            (Cell::Number(f), Cell::Number(y)) => f * y * y,
            _ => panic!(),
        })
        .collect();
    assert!((values.iter().sum::<f64>() - 20.0).abs() < 1e-9);
    let y: Vec<f64> = q
        .rows
        .iter()
        .map(|r| match r[2] {
            Cell::Number(y) => y,
            _ => panic!(),
        })
        .collect();
    assert!(y.windows(2).all(|w| w[0] <= w[1] + 1e-12), "{y:?}");
    let t = tables(
        "CATPCA taste price service noise /ANALYSIS=taste price service noise (LEVEL=NOMI)
         /CRITITER=1e-12 /MAXITER=5000 /PRINT=LOADING.",
    );
    check_solution(
        &t,
        [2.94525769, 0.85960090],
        [
            [0.94108602, -0.27671589],
            [0.93229750, 0.27267154],
            [0.78363284, -0.58166030],
            [0.75918096, 0.60856449],
        ],
        1e-5,
    );
}

#[test]
fn spline_levels_follow_the_alternating_least_squares_script() {
    let t = tables(
        "CATPCA taste price service noise /ANALYSIS=taste (LEVEL=SPORD INKNOT=1)
           price (LEVEL=SPNOM DEGREE=1 INKNOT=1) service (LEVEL=ORDI) noise (LEVEL=NUME)
         /CRITITER=1e-12 /MAXITER=5000 /PRINT=LOADING QUANT(taste price).",
    );
    check_solution(
        &t,
        [2.95862835, 0.80416882],
        [
            [0.93904038, -0.27880095],
            [0.92478334, 0.24947346],
            [0.81456749, -0.52978893],
            [0.74705228, 0.61929439],
        ],
        1e-5,
    );
    let quant = |var: &str, code: &str| {
        value(
            &t,
            &format!("Quantifications: {var}"),
            code,
            "Quantification",
        )
    };
    let taste = [
        -1.3220292569,
        -1.0291748923,
        -0.1506117985,
        0.8199922209,
        1.3889693622,
    ];
    let price = [
        -1.409000171,
        -0.6182714323,
        0.1724573063,
        0.8554900539,
        1.5385228016,
    ];
    for code in 1..=5 {
        let c = code.to_string();
        assert!((quant("taste", &c) - taste[code - 1]).abs() < 1e-5);
        assert!((quant("price", &c) - price[code - 1]).abs() < 1e-5);
    }
}

#[test]
fn multiple_nominal_variables_add_centroids() {
    let t = tables(
        "CATPCA taste price service noise area /ANALYSIS=taste price service noise (LEVEL=NUME)
           area (LEVEL=MNOM) /CRITITER=1e-12 /MAXITER=5000 /PRINT=LOADING VAF CORR QUANT(area).",
    );
    check_solution(
        &t,
        [3.88303151, 1.26451618],
        [
            [0.91255255, -0.21758208],
            [0.91178395, 0.16625639],
            [0.83134714, -0.36776158],
            [0.78279443, 0.44028931],
        ],
        1e-5,
    );
    // the total averages the multiple nominal part over the dimensions
    let centroid = |s: &str| {
        value(
            &t,
            "Variance Accounted For",
            "area",
            &format!("Centroid Coordinates / {s}"),
        )
    };
    let mean = (centroid(D1) + centroid(D2)) / 2.0;
    let vector_total = value(
        &t,
        "Variance Accounted For",
        "Active Total",
        "Vector Coordinates / Total",
    );
    check(
        &t,
        "Model Summary",
        "Total",
        EIGEN,
        vector_total + mean,
        1e-9,
    );
    // one correlation matrix per dimension, and no loadings for area
    assert!(titles(&t).contains(&"Correlations Transformed Variables: Dimension 2"));
    assert!(
        table(&t, "Component Loadings")
            .rows
            .iter()
            .all(|r| r[0] != Cell::Text("area".into()))
    );
    // the centroids of area weighted by their counts sum to zero in each dimension
    let q = table(&t, "Quantifications: area");
    for c in 2..4 {
        let sum: f64 = q
            .rows
            .iter()
            .map(|r| match (&r[1], &r[c]) {
                (Cell::Number(f), Cell::Number(x)) => f * x,
                _ => panic!(),
            })
            .sum();
        assert!(sum.abs() < 1e-9);
    }
}

#[test]
fn passive_gaps_case_weights_and_variable_weights() {
    let t = run(
        &mut Engine::default(),
        &format!(
            "{GAPS}CATPCA taste price service noise /ANALYSIS=taste (WEIGHT=2 LEVEL=ORDI) price (LEVEL=ORDI)
               service (LEVEL=NUME) noise (LEVEL=NOMI) /CRITITER=1e-12 /MAXITER=5000
             /PRINT=LOADING CORR."
        ),
    );
    check_solution(
        &t,
        [3.79917012, 1.06144385],
        [
            [0.97092941, -0.23137145],
            [0.87945069, 0.41718466],
            [0.86100347, -0.45832090],
            [0.63166586, 0.75516705],
        ],
        1e-5,
    );
    check(
        &t,
        "Case Processing Summary",
        "Active Cases with Missing Values",
        "N",
        3.0,
        0.0,
    );
    // with passive gaps the correlation eigenvalues no longer equal the fit
    let r = value(
        &t,
        "Correlations Transformed Variables",
        "Eigenvalue",
        "taste",
    );
    assert!(r > 0.0 && (r - 3.79917012).abs() > 1e-6);
}

#[test]
fn normalizations_keep_scores_times_loadings() {
    let syntax = |norm: &str| {
        format!(
            "CATPCA taste price service noise /ANALYSIS=taste price service noise (LEVEL=NUME)
             /NORMALIZATION={norm} /PRINT=LOADING OBJECT."
        )
    };
    let v = tables(&syntax("VPRINCIPAL"));
    let o = tables(&syntax("OPRINCIPAL"));
    let s = tables(&syntax("SYMMETRICAL"));
    let e1 = value(&v, "Model Summary", "1", EIGEN);
    // object principal: scores scaled by the root of the eigenvalue, loadings divided by it
    for case in ["1", "7", "20"] {
        let ratio = value(&o, "Object Scores", case, D1) / value(&v, "Object Scores", case, D1);
        assert!((ratio - e1.sqrt()).abs() < 1e-9);
    }
    let product = |t: &[Table]| {
        value(t, "Object Scores", "3", D1) * value(t, "Component Loadings", "taste", D1)
            + value(t, "Object Scores", "3", D2) * value(t, "Component Loadings", "taste", D2)
    };
    assert!((product(&v) - product(&o)).abs() < 1e-9);
    assert!((product(&v) - product(&s)).abs() < 1e-9);
    // variable principal scores are standardized
    let ss: f64 = (1..=20)
        .map(|c| value(&v, "Object Scores", &c.to_string(), D1).powi(2))
        .sum();
    assert!((ss - 20.0).abs() < 1e-9);
}

#[test]
fn varimax_turns_loadings_and_scores_together() {
    let t = tables(
        "CATPCA taste price service noise /ANALYSIS=taste price service noise (LEVEL=NUME)
         /ROTATION=VARIMAX /PRINT=LOADING OBJECT.",
    );
    // the varimax solution of the principal component loadings, with Kaiser normalization
    let expected = [
        [0.8891460860, 0.3867949812],
        [0.5322522032, 0.7904967789],
        [0.9312892922, 0.2074752616],
        [0.1908004012, 0.9492219699],
    ];
    for (name, l) in VARS.iter().zip(expected) {
        check(&t, "Rotated Component Loadings", name, D1, l[0], 1e-4);
        check(&t, "Rotated Component Loadings", name, D2, l[1], 1e-4);
    }
    assert!(titles(&t).contains(&"Component Transformation Matrix"));
    // rotated scores times rotated loadings reproduce the unrotated approximation
    let approx = |case: &str, var: &str, which: &str| {
        let scores = |d: &str| value(&t, "Object Scores", case, d);
        scores(D1) * value(&t, which, var, D1) + scores(D2) * value(&t, which, var, D2)
    };
    let plain = tables(
        "CATPCA taste price service noise /ANALYSIS=taste price service noise (LEVEL=NUME)
         /PRINT=LOADING OBJECT.",
    );
    let unrotated = value(&plain, "Object Scores", "5", D1)
        * value(&plain, "Component Loadings", "noise", D1)
        + value(&plain, "Object Scores", "5", D2)
            * value(&plain, "Component Loadings", "noise", D2);
    assert!((approx("5", "noise", "Rotated Component Loadings") - unrotated).abs() < 1e-9);
    // an oblique rotation shows pattern, structure and component correlations
    let t = tables(
        "CATPCA taste price service noise /ANALYSIS=taste price service noise (LEVEL=NUME)
         /ROTATION=OBLIMIN NOKAISER /PRINT=LOADING.",
    );
    for title in [
        "Pattern Matrix",
        "Structure Matrix",
        "Component Correlation Matrix",
    ] {
        assert!(titles(&t).contains(&title), "{title}");
    }
    check(&t, "Component Correlation Matrix", "1", "1", 1.0, 1e-9);
}

#[test]
fn supplementary_objects_and_variables_and_a_fixed_configuration() {
    let t = tables(
        "CATPCA taste price service noise area /ANALYSIS=taste price service noise (LEVEL=NUME) area (LEVEL=ORDI)
         /SUPPLEMENTARY=OBJECT(20) VARIABLE(area) /PRINT=LOADING OBJECT VAF.",
    );
    check(
        &t,
        "Case Processing Summary",
        "Supplementary Cases",
        "N",
        1.0,
        0.0,
    );
    let scores = table(&t, "Object Scores");
    assert_eq!(scores.rows.len(), 20);
    assert!(scores.caption.as_deref().unwrap().contains("20"));
    // the supplementary variable has a loading but takes no part in the total
    let area = value(&t, "Component Loadings", "area (supplementary)", D1);
    assert!(area.abs() > 0.5 && area.abs() <= 1.0);
    let total = value(
        &t,
        "Variance Accounted For",
        "Active Total",
        "Vector Coordinates / Total",
    );
    check(&t, "Model Summary", "Total", EIGEN, total, 1e-9);
    // the scores as a fixed configuration: the variables get their loadings back
    let mut engine = Engine::default();
    let syntax = "CATPCA taste price service noise /ANALYSIS=taste price service noise (LEVEL=NUME)
       /PLOT=NONE /PRINT=LOADING";
    let base = run(
        &mut engine,
        &format!(
            "{DATA}DATASET NAME visits.\nDATASET DECLARE scores.\n{syntax} /OUTFILE=OBJECT(scores)."
        ),
    );
    let t = run(
        &mut engine,
        &format!("{syntax} /CONFIGURATION=FIXED(scores)."),
    );
    assert!(!titles(&t).contains(&"Model Summary"));
    for v in VARS {
        let expected = value(&base, "Component Loadings", v, D2);
        check(
            &t,
            "Component Loadings",
            &format!("{v} (supplementary)"),
            D2,
            expected,
            1e-9,
        );
    }
}

#[test]
fn save_and_outfile_name_their_variables() {
    let mut engine = Engine::default();
    run(
        &mut engine,
        &format!(
            "{DATA}DATASET NAME visits.
             DATASET DECLARE scores.
             DATASET DECLARE quant.
             DATASET DECLARE fitted.
             DATASET DECLARE codes.
             CATPCA taste price service noise area /ANALYSIS=taste price service noise (LEVEL=ORDI) area (LEVEL=MNOM)
             /SAVE=TRDATA OBJECT APPROX /OUTFILE=OBJECT(scores) TRDATA(quant) APPROX(fitted) DISCRDATA(codes)
             /PRINT=NONE /PLOT=NONE.
             CATPCA taste price /ANALYSIS=taste price (LEVEL=NUME) /SAVE=OBJECT(SCORE(1)) /PRINT=NONE /PLOT=NONE."
        ),
    );
    let data = engine.dataset().unwrap();
    for name in [
        "TRA1_1", "TRA4_1", "TRA5_1_1", "TRA5_2_1", "OBSCO1_1", "OBSCO2_1", "APP1_1", "APP5_1",
        "SCORE1_1",
    ] {
        assert!(data.variable_index(name).is_some(), "{name}");
    }
    assert!(data.variable_index("SCORE2_1").is_none());
    let tra: Vec<f64> = match data.variable("TRA2_1").unwrap().values() {
        Values::Numeric(v) => v.iter().map(|x| x.unwrap()).collect(),
        _ => panic!(),
    };
    // the saved quantified variable has mean 0 and sum of squares n
    assert!(tra.iter().sum::<f64>().abs() < 1e-9);
    assert!((tra.iter().map(|x| x * x).sum::<f64>() - 20.0).abs() < 1e-9);
    for (file, name) in [
        ("scores", "OBSCO2"),
        ("quant", "TRA5_2"),
        ("fitted", "APP3"),
        ("codes", "area"),
    ] {
        run(&mut engine, &format!("DATASET ACTIVATE {file}."));
        let set = engine.dataset().unwrap();
        assert!(set.variable_index(name).is_some(), "{file} {name}");
        assert!(set.variable_index("TRA1").is_some() == (file == "quant"));
    }
}

#[test]
fn charts_projected_centroids_warnings_and_errors() {
    let mut engine = Engine::default();
    let t = run(
        &mut engine,
        &format!(
            "{DATA}CATPCA taste price service noise area /ANALYSIS=taste price service noise (LEVEL=ORDI) area (LEVEL=MNOM)
             /PRINT=NONE /PLOT=OBJECT(area) LOADING((CENTR)) CATEGORY(taste) JOINTCAT TRANS(taste area(2))
               RESID(price) BIPLOT TRIPLOT PROJCENTR(area, taste price) VAF LDELLAREA NDIM(1,2)."
        ),
    );
    assert!(titles(&t).contains(&"Projected Centroids: area"));
    let charts: Vec<String> = engine.take_charts().into_iter().map(|c| c.title).collect();
    for title in [
        "Object Points Labeled by area, Dimension 1 by 2",
        "Component Loadings and Centroids, Dimension 1 by 2",
        "Transformation: area, Dimension 2",
        "Residuals: price",
        "Variance Accounted For: Total",
        "Projected Centroids of area",
    ] {
        assert!(charts.iter().any(|c| c == title), "{title} in {charts:?}");
    }
    assert!(engine.warnings().iter().any(|w| w.contains("LDELLAREA")));
    assert!(
        run_error(&format!(
            "{DATA}CATPCA taste area /ANALYSIS=taste area (LEVEL=MNOM) /PLOT=PROJCENTR(taste, area)."
        ))
        .contains("multiple nominal")
    );
    assert!(run_error(&format!("{DATA}CATPCA taste /ANALYSIS=taste.")).contains("two variables"));
}

/// The numbers of row `row` of a bootstrap table, after its label columns.
fn numbers_of(tables: &[Table], title: &str, row: usize, labels: usize) -> Vec<Option<f64>> {
    table(tables, title).rows[row][labels..]
        .iter()
        .map(|c| match c {
            Cell::Number(x) => Some(*x),
            _ => None,
        })
        .collect()
}

const BOOT_COMMAND: &str =
    "CATPCA taste price service noise /ANALYSIS=taste price service noise (LEVEL=NUME)
     /PRINT=LOADING";

#[test]
fn bootstrap_tables_hold_the_data_and_intervals_around_it() {
    let t = tables(&format!(
        "SET SEED=11.\n{BOOT_COMMAND} /RESAMPLE=BOOTSTRAP(60, 90, UNBALANCED, REFLEC)."
    ));
    // the eigenvalues of the data come first, the intervals have the asked percent in the header
    let eigen = table(&t, "Bootstrap Eigenvalues");
    assert_eq!(eigen.rows.len(), 2);
    assert_eq!(
        eigen.column_titles(),
        [
            "Dimension",
            "Data",
            "Mean",
            "Std. Deviation",
            "90% Percentile Interval / Lower",
            "90% Percentile Interval / Upper"
        ]
    );
    let first = numbers_of(&t, "Bootstrap Eigenvalues", 0, 1);
    let Cell::Number(model_eigen) = table(&t, "Model Summary").rows[0][2] else {
        panic!("no eigenvalue")
    };
    assert!((model_eigen - first[0].unwrap()).abs() < 1e-9);
    let (lower, upper) = (first[3].unwrap(), first[4].unwrap());
    assert!(
        lower < upper && lower > 2.0 && upper < 4.0,
        "{lower} {upper}"
    );
    // the loadings of the data are those of the Component Loadings table
    let loadings = table(&t, "Bootstrap Component Loadings");
    assert_eq!(loadings.rows.len(), 8, "four variables in two dimensions");
    for (v, name) in VARS.iter().enumerate() {
        for s in 0..2 {
            let row = numbers_of(&t, "Bootstrap Component Loadings", v * 2 + s, 2);
            let shown = value(&t, "Component Loadings", name, [D1, D2][s]);
            assert!((row[0].unwrap() - shown).abs() < 1e-9, "{name} {s}");
            assert!(row[3].unwrap() <= row[4].unwrap());
        }
    }
    // a category value: counted in at most the 60 samples
    let q = table(&t, "Bootstrap Quantifications");
    assert_eq!(
        q.rows.len(),
        19,
        "the categories of the four variables (noise has four)"
    );
    for row in &q.rows {
        let Cell::Number(count) = row[row.len() - 1] else {
            panic!("no count")
        };
        assert!((1.0..=60.0).contains(&count));
    }
    assert!(
        table(&t, "Bootstrap Eigenvalues")
            .caption
            .as_deref()
            .unwrap()
            .contains("60 bootstrap samples")
    );
}

#[test]
fn bootstrap_is_repeatable_under_a_seed_and_differs_without_it() {
    let command = format!("{BOOT_COMMAND} /RESAMPLE=BOOTSTRAP(30).");
    let run_with = |seed: u32| {
        let t = tables(&format!("SET SEED={seed}.\n{command}"));
        numbers_of(&t, "Bootstrap Component Loadings", 0, 2)
    };
    assert_eq!(run_with(5), run_with(5));
    assert_ne!(run_with(5), run_with(6));
    // unbalanced samples are another set of samples
    let balanced = tables(&format!("SET SEED=5.\n{command}"));
    let unbalanced = tables(&format!(
        "SET SEED=5.\n{BOOT_COMMAND} /RESAMPLE=BOOTSTRAP(30, UNBALANCED)."
    ));
    assert_ne!(
        numbers_of(&balanced, "Bootstrap Component Loadings", 0, 2),
        numbers_of(&unbalanced, "Bootstrap Component Loadings", 0, 2)
    );
}

/// Four variables that are one factor with a little noise: the solution hardly moves under
/// resampling, so the intervals are narrow and lie around the data's loadings.
#[test]
fn bootstrap_intervals_are_narrow_for_nearly_noiseless_data() {
    let mut rows = String::new();
    for i in 0..40 {
        let f = f64::from(i) / 4.0;
        let noise = |k: u32| 0.002 * f64::from((i * 7 + k * 13) % 11) - 0.01;
        rows += &format!(
            "{:.4} {:.4} {:.4} {:.4}\n",
            f + noise(1),
            2.0 * f + 1.0 + noise(2),
            -1.5 * f + noise(3),
            0.5 * f + 3.0 + noise(4)
        );
    }
    let data = format!("DATA LIST FREE / a b c d.\nBEGIN DATA\n{rows}END DATA.\n");
    let mut engine = Engine::default();
    let t = run(
        &mut engine,
        &format!(
            "{data}SET SEED=3.\nCATPCA a b c d /ANALYSIS=a b c d (LEVEL=NUME) /DIMENSION=1 /PRINT=LOADING
             /RESAMPLE=BOOTSTRAP(80, 95, BALANCED, PROCRU)."
        ),
    );
    let eigen = numbers_of(&t, "Bootstrap Eigenvalues", 0, 1);
    assert!(eigen[0].unwrap() > 3.99);
    assert!(eigen[4].unwrap() - eigen[3].unwrap() < 0.02, "{eigen:?}");
    for v in 0..4 {
        let row = numbers_of(&t, "Bootstrap Component Loadings", v, 2);
        let (data_value, mean, lower, upper) = (
            row[0].unwrap(),
            row[1].unwrap(),
            row[3].unwrap(),
            row[4].unwrap(),
        );
        assert!(upper - lower < 0.01, "variable {v}: {lower} {upper}");
        assert!((mean - data_value).abs() < 0.005, "variable {v}");
        // the loading of a and c have opposite signs (c falls as a rises)
        assert!(data_value.abs() > 0.99);
    }
    let sign = |v: usize| {
        numbers_of(&t, "Bootstrap Component Loadings", v, 2)[0]
            .unwrap()
            .signum()
    };
    assert_ne!(sign(0), sign(2));
}

#[test]
fn bootstrap_refusals() {
    assert!(
        run_error(&format!(
            "{DATA}CATPCA taste area /ANALYSIS=taste area (LEVEL=MNOM) /RESAMPLE=BOOTSTRAP(5)."
        ))
        .contains("not multiple nominal")
    );
    let mut engine = Engine::default();
    let t = run(
        &mut engine,
        &format!("{DATA}SET SEED=2.\n{BOOT_COMMAND} /RESAMPLE=BOOTSTRAP(8) /PLOT=LDELLAREA."),
    );
    assert!(titles(&t).contains(&"Bootstrap Eigenvalues"));
    assert!(engine.warnings().iter().any(|w| w.contains("LDELLAREA")));
}

fn chart_titles(engine: &mut Engine) -> Vec<String> {
    engine.take_charts().into_iter().map(|c| c.title).collect()
}

#[test]
fn a_two_dimensional_bootstrap_draws_ellipses_the_eigenvalue_plot_and_value_bands() {
    let mut engine = Engine::default();
    run(
        &mut engine,
        &format!(
            "{DATA}SET SEED=4.\n{BOOT_COMMAND} /RESAMPLE=BOOTSTRAP(40)
             /PLOT=OBJECT LOADING CATEGORY(taste) JOINTCAT TRANS(taste) OBELLAREA(> AREA 0) NELLPNT(12)."
        ),
    );
    let charts = engine.take_charts();
    let titles: Vec<&str> = charts.iter().map(|c| c.title.as_str()).collect();
    for want in [
        "Bootstrap Eigenvalues",
        "Confidence Ellipse of the Eigenvalues",
        "Confidence Ellipses of the Component Loadings",
        "Confidence Ellipses of the Object Points",
        "Confidence Ellipses of the Category Points: taste",
        "Confidence Ellipses of the Category Points",
    ] {
        assert!(titles.contains(&want), "{want} in {titles:?}");
    }
    // the eigenvalue plot: the data and its interval; the transformation plot: value and bands
    let eigen = charts
        .iter()
        .find(|c| c.title == "Bootstrap Eigenvalues")
        .unwrap();
    assert_eq!(
        eigen
            .series
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>(),
        ["Data", "Lower", "Upper"]
    );
    for k in 0..2 {
        assert!(eigen.series[1].values[k] <= eigen.series[2].values[k]);
    }
    let trans = charts
        .iter()
        .find(|c| c.title == "Transformation: taste")
        .unwrap();
    let names: Vec<&str> = trans.series.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["Quantification", "Lower", "Upper"]);
    for k in 0..trans.series[0].values.len() {
        let (lower, upper) = (trans.series[1].values[k], trans.series[2].values[k]);
        if lower.is_finite() {
            assert!(lower <= upper);
        }
    }
    // an ellipse is its point and a closed contour of NELLPNT + 1 points; 20 objects, 20 ellipses
    let objects = charts
        .iter()
        .find(|c| c.title == "Confidence Ellipses of the Object Points")
        .unwrap();
    let Some(statscraft_engine::plot::Plot::Points { groups, .. }) = &objects.plot else {
        panic!("no points")
    };
    assert_eq!(groups.len(), 20);
    assert!(groups.iter().all(|g| g.points.len() == 1 + 13));
    assert_eq!(groups[0].points[1], groups[0].points[13]);
}

#[test]
fn ellipse_limits_choose_which_ellipses_are_drawn_and_a_seed_repeats_them() {
    let run_with = |plot: &str| {
        let mut engine = Engine::default();
        run(
            &mut engine,
            &format!(
                "{DATA}SET SEED=9.\n{BOOT_COMMAND} /RESAMPLE=BOOTSTRAP(30) /PLOT=LOADING {plot}."
            ),
        );
        engine.take_charts()
    };
    let groups_of = |charts: &[statscraft_engine::Chart]| -> usize {
        let c = charts
            .iter()
            .find(|c| c.title == "Confidence Ellipses of the Component Loadings");
        match c.and_then(|c| c.plot.as_ref()) {
            Some(statscraft_engine::plot::Plot::Points { groups, .. }) => groups.len(),
            _ => 0,
        }
    };
    // by default every loading's ellipse has an area above 0
    let all = run_with("");
    assert_eq!(groups_of(&all), 4);
    // an impossible limit leaves no chart; a limit of the area below an enormous value all
    assert_eq!(groups_of(&run_with("LDELLAREA(> AREA 1000)")), 0);
    assert_eq!(groups_of(&run_with("LDELLAREA(< AREA 1000)")), 4);
    // the same seed gives the same ellipses
    assert_eq!(run_with("LDELLAREA"), all);
}

#[test]
fn ellipses_need_a_two_dimensional_solution() {
    let mut engine = Engine::default();
    run(
        &mut engine,
        &format!(
            "{DATA}SET SEED=4.\n{BOOT_COMMAND} /DIMENSION=3 /RESAMPLE=BOOTSTRAP(10) /PLOT=LOADING."
        ),
    );
    let titles = chart_titles(&mut engine);
    assert!(titles.iter().any(|t| t == "Bootstrap Eigenvalues"));
    assert!(!titles.iter().any(|t| t.starts_with("Confidence Ellipse")));
}
