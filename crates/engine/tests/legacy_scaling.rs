//! HOMALS, PRINCALS, ANACOR and OVERALS on invented data. The first three must give the
//! numbers of MULTIPLE CORRESPONDENCE, CATPCA and CORRESPONDENCE on the same model. The
//! OVERALS numbers come from a separate base R script that follows the steps of the
//! Algorithms manual with indicator matrices (nested numerical start, Procrustes
//! orthonormalization, principal axes at the end), converged to 1e-14; the canonical
//! correlations come from base R's cancor.

use statscraft_data::Values;
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

fn run(engine: &mut Engine, source: &str) -> Result<Vec<Table>, String> {
    let (commands, errors) = parse(source);
    if let Some(e) = errors.first() {
        return Err(e.message.clone());
    }
    let mut tables = Vec::new();
    for command in &commands {
        tables.extend(engine.execute(&command.command).map_err(|e| e.0)?);
    }
    Ok(tables)
}

fn tables(source: &str) -> Vec<Table> {
    run(&mut Engine::default(), source).unwrap_or_else(|e| panic!("{e}"))
}

fn error(source: &str) -> String {
    run(&mut Engine::default(), source).expect_err("an error")
}

fn table<'t>(tables: &'t [Table], title: &str) -> &'t Table {
    let titles: Vec<&str> = tables.iter().map(|t| t.title.as_str()).collect();
    tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title} in {titles:?}"))
}

/// The number in the row with `row` among its first two cells and the column `column`.
fn value(tables: &[Table], title: &str, row: &str, column: &str) -> f64 {
    let t = table(tables, title);
    let r = t
        .rows
        .iter()
        .position(|r| r.iter().take(2).any(|c| *c == Cell::Text(row.into())))
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

/// Every number of `title` in both outputs agrees.
fn same_table(a: &[Table], b: &[Table], title: &str) {
    let (ta, tb) = (table(a, title), table(b, title));
    assert_eq!(ta.rows.len(), tb.rows.len(), "{title}");
    for (ra, rb) in ta.rows.iter().zip(&tb.rows) {
        for (ca, cb) in ra.iter().zip(rb) {
            match (ca, cb) {
                (Cell::Number(x), Cell::Number(y)) => {
                    assert!((x - y).abs() < 1e-9, "{title}: {x} against {y}")
                }
                _ => assert_eq!(ca, cb, "{title}"),
            }
        }
    }
}

/// Twelve shoppers: brand (1-3), shop (1-4), pay (1-2) and a price class (1-4).
const SHOP: &str = "DATA LIST FREE / brand shop pay price.
BEGIN DATA
1 1 1 2
2 3 2 4
1 2 1 1
3 4 2 3
2 2 1 3
3 3 2 4
1 1 2 1
2 4 1 2
3 2 2 4
1 3 1 2
2 1 2 3
3 4 1 4
END DATA.
";

#[test]
fn homals_gives_the_numbers_of_multiple_correspondence() {
    let old = tables(&format!(
        "{SHOP}HOMALS VARIABLES=brand(3) shop(4) pay(2) /PRINT=ALL /PLOT=NONE."
    ));
    let new = tables(&format!(
        "{SHOP}MULTIPLE CORRESPONDENCE brand shop pay /ANALYSIS=brand shop pay
           /PRINT=DESCRIP DISCRIM QUANT OBJECT /PLOT=NONE."
    ));
    for title in [
        "Model Summary",
        "Discrimination Measures",
        "Object Scores",
        "Quantifications: shop",
        "brand",
    ] {
        same_table(&old, &new, title);
    }
    // PRINT=ALL adds the history; the correlations of MULTIPLE CORRESPONDENCE are not shown
    assert!(old.iter().any(|t| t.title == "Iteration History"));
    assert!(!old.iter().any(|t| t.title.starts_with("Correlations")));
}

#[test]
fn homals_reads_categories_one_to_the_largest() {
    // 0 and 5 are outside shop(4), 2.7 is category 2 and the user-missing 3 still counts
    let data = "DATA LIST FREE / brand shop pay.
BEGIN DATA
1 1 1
2 3 2
1 2.7 1
3 4 2
2 0 1
3 3 2
1 1 2
2 4 1
3 5 2
1 3 1
END DATA.
MISSING VALUES shop (3).
";
    let old = tables(&format!(
        "{data}HOMALS brand(3) shop(4) pay(2) /PRINT=FREQ EIGEN DISCRIM /PLOT=NONE."
    ));
    let recoded = "DATA LIST FREE / brand shop pay.
BEGIN DATA
1 1 1
2 3 2
1 2 1
3 4 2
2 . 1
3 3 2
1 1 2
2 4 1
3 . 2
1 3 1
END DATA.
";
    let new = tables(&format!(
        "{recoded}MULTIPLE CORRESPONDENCE brand shop pay /ANALYSIS=brand shop pay
           /PRINT=DESCRIP DISCRIM /PLOT=NONE."
    ));
    same_table(&old, &new, "Model Summary");
    same_table(&old, &new, "Discrimination Measures");
    check(&old, "shop", "3", "Frequency", 3.0, 0.0);
    check(&old, "shop", "Missing", "Frequency", 2.0, 0.0);
}

#[test]
fn homals_nobservations_save_and_matrix() {
    let mut engine = Engine::default();
    let first = run(
        &mut engine,
        &format!(
            "{SHOP}DATASET DECLARE quants.\n HOMALS brand shop(4) /NOBSERVATIONS=10 /DIMENSION=2 /SAVE=sc(1)
               /MATRIX=OUT(quants) /PRINT=OBJECT."
        ),
    )
    .unwrap();
    let ten = tables(&format!(
        "{SHOP}SELECT IF $CASENUM <= 10.
         MULTIPLE CORRESPONDENCE brand shop /ANALYSIS=brand shop /PRINT=OBJECT /PLOT=NONE."
    ));
    same_table(&first, &ten, "Object Scores");
    let data = engine.dataset().unwrap();
    let saved: Vec<Option<f64>> = match data.variable("sc1_1").unwrap().values() {
        Values::Numeric(v) => v.iter().collect(),
        Values::String(_) => panic!("a string"),
    };
    assert!(saved[9].is_some() && saved[11].is_none());
    assert!(data.variable_index("sc2_1").is_none());
    // the matrix dataset: one case per category of brand (3) and shop (4)
    run(&mut engine, "DATASET ACTIVATE quants.").unwrap();
    let m = engine.dataset().unwrap();
    assert_eq!(m.case_count(), 7);
    let names: Vec<&str> = m.variable_names().collect();
    assert_eq!(names, ["ROWTYPE_", "LEVEL", "VARNAME_", "DIM1", "DIM2"]);
}

#[test]
fn princals_gives_the_numbers_of_catpca() {
    let old = tables(&format!(
        "{SHOP}PRINCALS VARIABLES=brand(3) shop(4) pay(2) price(4)
           /ANALYSIS=brand(SNOM) shop(MNOM) pay price(NUME)
           /PRINT=ALL /PLOT=NONE."
    ));
    let new = tables(&format!(
        "{SHOP}CATPCA brand shop pay price
           /ANALYSIS=brand(LEVEL=NOMI) shop(LEVEL=MNOM) pay(LEVEL=ORDI) price(LEVEL=NUME)
           /PRINT=DESCRIP LOADING CORR QUANT OBJECT /PLOT=NONE."
    ));
    for title in [
        "Model Summary",
        "Component Loadings",
        "Object Scores",
        "Quantifications: brand",
        "Quantifications: shop",
    ] {
        same_table(&old, &new, title);
    }
}

#[test]
fn princals_matrix_replaces_the_active_dataset() {
    let mut engine = Engine::default();
    run(
        &mut engine,
        &format!(
            "{SHOP}PRINCALS brand(3) price(4) pay(2) /ANALYSIS=brand(SNOM) price pay(NUME)
               /MATRIX=OUT(*) /SAVE /PLOT=NONE."
        ),
    )
    .unwrap();
    let m = engine.dataset().unwrap();
    // per single variable: QUANT and SCOOR_ per category, then LOADING_
    assert_eq!(m.case_count(), 2 * (3 + 4 + 2) + 3);
    assert!(m.variable_index("VARTYPE_").is_some());
    assert!(m.variable_index("PRI1_1").is_none());
}

#[test]
fn anacor_gives_the_numbers_of_correspondence() {
    let data = "DATA LIST FREE / region channel count.
BEGIN DATA
1 1 30
1 2 12
1 3 8
2 1 10
2 2 25
2 3 15
3 1 6
3 2 9
3 3 31
4 1 14
4 2 14
4 3 12
END DATA.
WEIGHT BY count.
";
    for (old, new) in [
        ("", "SYMMETRICAL"),
        ("/NORMALIZATION=RPRINCIPAL", "RPRINCIPAL"),
        ("/NORMALIZATION=PRINCIPAL", "PRINCIPAL"),
        ("/NORMALIZATION=-0.5", "-0.5"),
    ] {
        let a = tables(&format!(
            "{data}ANACOR TABLE=region(1,4) BY channel(1,3) {old} /PRINT=SCORES /PLOT=NONE."
        ));
        let b = tables(&format!(
            "{data}CORRESPONDENCE TABLE=region(1,4) BY channel(1,3) /NORMALIZATION={new}
               /PRINT=RPOINTS CPOINTS /PLOT=NONE."
        ));
        for title in ["Summary", "Overview Row Points", "Overview Column Points"] {
            same_table(&a, &b, title);
        }
    }
    // q beyond 1: the rows get the power 1.5 of the singular value, the columns -0.5
    let wide = tables(&format!(
        "{data}ANACOR TABLE=region(1,4) BY channel(1,3) /NORMALIZATION=2 /PRINT=SCORES /PLOT=NONE."
    ));
    let principal = tables(&format!(
        "{data}ANACOR TABLE=region(1,4) BY channel(1,3) /NORMALIZATION=RPRINCIPAL /PRINT=SCORES /PLOT=NONE."
    ));
    let singular = value(&wide, "Summary", "1", "Singular Value");
    let col = "Score in Dimension / 1";
    let ratio = value(&wide, "Overview Row Points", "1", col)
        / value(&principal, "Overview Row Points", "1", col);
    assert!((ratio - singular.sqrt()).abs() < 1e-9, "{ratio}");
}

/// Fourteen invented cases in three sets; c is out of range for case 5, d for case 9.
const SETS: &str = "DATA LIST FREE / a b c d e.
BEGIN DATA
1 1 2 1 1
2 1 1 2 2
2 2 3 2 2
3 3 4 3 3
4 2 0 3 4
1 1 1 1 2
3 3 4 2 3
4 3 5 3 4
2 2 2 9 1
1 1 2 1 1
3 2 3 3 3
4 3 5 3 4
2 1 1 2 2
3 2 3 2 3
END DATA.
";

const OVERALS_RUN: &str = "OVERALS VARIABLES=a(4) b(3) c(5) d(3) e(4)
  /ANALYSIS=a(ORDI) b(MNOM) c(NUME) d(SNOM) e(NUME) /SETS=3(2,1,2)
  /CONVERGENCE=1E-14 /MAXITER=20000 /PRINT=FREQ QUANT CENTROID WEIGHTS OBJECT FIT
  /PLOT=NONE.";

#[test]
fn overals_matches_the_reference_computation() {
    let t = tables(&format!("{SETS}{OVERALS_RUN}"));
    let s = "Summary of Analysis";
    let (d1, d2) = ("Dimension 1", "Dimension 2");
    check(&t, s, "Eigenvalue", d1, 0.8917574377, 1e-6);
    check(&t, s, "Eigenvalue", d2, 0.6293576772, 1e-6);
    check(&t, s, "Loss Set 1", d1, 0.0757962237, 1e-6);
    check(&t, s, "Loss Set 2", d2, 0.9716882528, 1e-6);
    check(&t, s, "Loss Set 3", d1, 0.1585991107, 1e-6);
    check(
        &t,
        s,
        "Eigenvalue",
        "Sum",
        0.8917574377 + 0.6293576772,
        1e-6,
    );
    let o = "Object Scores";
    check(&t, o, "1", d1, -1.2473404413, 1e-6);
    check(&t, o, "1", d2, -1.3506329799, 1e-6);
    check(&t, o, "5", d1, 0.8664883519, 1e-6);
    check(&t, o, "9", d2, 0.4410259975, 1e-6);
    check(&t, o, "14", d1, 0.2821942330, 1e-6);
    check(&t, "Weights", "a", d2, 1.2914139118, 1e-6);
    check(&t, "Weights", "c", d1, 0.9352494769, 1e-6);
    check(&t, "Weights", "d", d1, 0.0107546252, 1e-6);
    check(&t, "Weights", "e", d2, -0.6798549420, 1e-6);
    check(&t, "Component Loadings", "a", d1, 0.6429046177, 1e-6);
    check(&t, "Component Loadings", "e", d2, 0.1189087689, 1e-6);
    let q = |v: &str| format!("Quantifications: {v}");
    check(&t, &q("a"), "1", "Quantification", -1.9566972850, 1e-6);
    check(&t, &q("a"), "4", "Quantification", 0.4780649637, 1e-6);
    check(&t, &q("d"), "2", "Quantification", 0.6614361815, 1e-6);
    check(&t, &q("d"), "3", "Quantification", 0.4341616513, 1e-6);
    check(&t, &q("c"), "5", "Quantification", 1.6929603946, 1e-6);
    check(
        &t,
        &q("b"),
        "1",
        "Multiple Coordinates / 1",
        -1.1268900171,
        1e-6,
    );
    check(
        &t,
        &q("b"),
        "1",
        "Multiple Coordinates / 2",
        1.0763996145,
        1e-6,
    );
    check(
        &t,
        &q("e"),
        "4",
        "Multiple Coordinates / 2",
        -0.8896166024,
        1e-6,
    );
    check(&t, "Centroids: d", "2", "Centroids / 2", 0.9690069003, 1e-6);
    check(&t, "Centroids: b", "3", "Centroids / 1", 1.2285002428, 1e-6);
    // single fit is the squared weight, single loss the multiple fit less it
    let a2 = 1.2914139118_f64.powi(2);
    check(&t, "Fit", "a", "Single Fit / Dimension 2", a2, 1e-5);
    let multiple = value(&t, "Fit", "a", "Multiple Fit / Dimension 2");
    check(
        &t,
        "Fit",
        "a",
        "Single Loss / Dimension 2",
        multiple - a2,
        1e-5,
    );
    // case 5 leaves set 2 out; case 9 leaves set 3 out
    check(
        &t,
        "Marginal Frequencies: c",
        "Missing",
        "Frequency",
        1.0,
        0.0,
    );
    check(
        &t,
        "Marginal Frequencies: e",
        "Missing",
        "Frequency",
        1.0,
        0.0,
    );
}

#[test]
fn overals_of_two_numerical_sets_is_canonical_correlation() {
    let data = "DATA LIST FREE / u1 u2 v1 v2.
BEGIN DATA
3 2 4 1
1 2 1 3
4 5 5 4
2 1 2 2
5 4 4 5
3 4 3 2
2 3 1 4
4 3 5 3
1 1 2 2
5 5 4 3
3 2 2 5
2 4 3 1
END DATA.
";
    let t = tables(&format!(
        "{data}OVERALS u1 u2 v1 v2(5) /ANALYSIS=u1 TO v2(NUME) /SETS=2(2,2)
           /CONVERGENCE=1E-14 /MAXITER=20000 /PLOT=NONE."
    ));
    // canonical correlations 0.899869742556 and 0.104440166262
    check(
        &t,
        "Summary of Analysis",
        "Eigenvalue",
        "Dimension 1",
        (1.0 + 0.899869742556) / 2.0,
        1e-7,
    );
    check(
        &t,
        "Summary of Analysis",
        "Eigenvalue",
        "Dimension 2",
        (1.0 + 0.104440166262) / 2.0,
        1e-7,
    );
    // one variable per set: (1 + |r|)/2 with r = 0.792485533786
    let t = tables(&format!(
        "{data}OVERALS u1 v1(5) /ANALYSIS=u1 v1(NUME) /SETS=2(1,1) /DIMENSION=1
           /CONVERGENCE=1E-14 /PLOT=NONE."
    ));
    check(
        &t,
        "Summary of Analysis",
        "Eigenvalue",
        "Dimension 1",
        (1.0 + 0.792485533786) / 2.0,
        1e-7,
    );
}

#[test]
fn overals_with_one_variable_per_set_is_homals_and_princals() {
    let homals = tables(&format!(
        "{SHOP}HOMALS brand(3) shop(4) pay(2) /PRINT=NONE /PLOT=NONE /CONVERGENCE=1E-12 /MAXITER=5000."
    ));
    let overals = tables(&format!(
        "{SHOP}OVERALS brand(3) shop(4) pay(2) /ANALYSIS=brand shop pay(MNOM) /SETS=3(1,1,1)
           /CONVERGENCE=1E-14 /MAXITER=20000 /PRINT=OBJECT /PLOT=NONE."
    ));
    for (row, d) in [("1", "Dimension 1"), ("2", "Dimension 2")] {
        let inertia = value(
            &homals,
            "Model Summary",
            row,
            "Variance Accounted For / Inertia",
        );
        check(
            &overals,
            "Summary of Analysis",
            "Eigenvalue",
            d,
            inertia,
            1e-6,
        );
    }
    let princals = tables(&format!(
        "{SHOP}PRINCALS brand(3) price(4) pay(2) /ANALYSIS=brand price pay(NUME) /PRINT=NONE
           /PLOT=NONE /CONVERGENCE=1E-12 /MAXITER=5000."
    ));
    let overals = tables(&format!(
        "{SHOP}OVERALS brand(3) price(4) pay(2) /ANALYSIS=brand price pay(NUME) /SETS=3(1,1,1)
           /CONVERGENCE=1E-14 /MAXITER=20000 /PLOT=NONE."
    ));
    for (row, d) in [("1", "Dimension 1"), ("2", "Dimension 2")] {
        let total = value(
            &princals,
            "Model Summary",
            row,
            "Variance Accounted For / Total (Eigenvalue)",
        );
        check(
            &overals,
            "Summary of Analysis",
            "Eigenvalue",
            d,
            total / 3.0,
            1e-6,
        );
    }
}

#[test]
fn overals_save_matrix_and_errors() {
    let mut engine = Engine::default();
    run(
        &mut engine,
        &format!(
            "{SETS}DATASET DECLARE mat.\n OVERALS VARIABLES=a(4) b(3) c(5) d(3) e(4)
               /ANALYSIS=a(ORDI) b(MNOM) c(NUME) d(SNOM) e(NUME) /SETS=3(2,1,2)
               /SAVE /MATRIX=OUT(mat) /PLOT=OBJECT(b) LOADINGS TRANS(a b) QUANT CENTROID(d)."
        ),
    )
    .unwrap();
    let data = engine.dataset().unwrap();
    assert!(data.variable_index("OVE1_1").is_some() && data.variable_index("OVE2_1").is_some());
    run(&mut engine, "DATASET ACTIVATE mat.").unwrap();
    let m = engine.dataset().unwrap();
    let names: Vec<&str> = m.variable_names().collect();
    assert_eq!(
        names,
        [
            "ROWTYPE_", "LEVEL", "VARNAME_", "VARTYPE_", "SET_", "DIM1", "DIM2"
        ]
    );
    // singles: 5 rows per category and 2 per variable; b (MNOM): 3 per category
    assert_eq!(m.case_count(), 5 * (4 + 5 + 3 + 4) + 2 * 4 + 3 * 3);
    assert!(
        error("DATA LIST FREE / a b.\nBEGIN DATA\n1 2\nEND DATA.\nOVERALS a b(2) /SETS=2(1,1).")
            .contains("ANALYSIS")
    );
    assert!(
        error(&format!("{SETS}OVERALS a b(3) /ANALYSIS=a b /SETS=2(1,2)."))
            .contains("SETS counts 3")
    );
    assert!(error(&format!("{SETS}HOMALS a(3) b(3) /ANALYSIS=a /ANALYSIS=b.")).contains("twice"));
    assert!(error(&format!("{SETS}HOMALS a b(3) /PLOT=DISCRIM(a).")).contains("no variable list"));
    assert!(
        error(&format!(
            "{SETS}ANACOR TABLE=a(1,4) BY b(1,3) /NORMALIZATION=3."
        ))
        .contains("-2 to 2")
    );
}

#[test]
fn abbreviations_charts_and_the_active_dataset_replaced() {
    let mut engine = Engine::default();
    let t = run(
        &mut engine,
        &format!(
            "{SHOP}HOMALS VAR=brand(3) shop(4) pay(2) /ANAL=brand shop /NOBS=11 /DIMENSIONS=3
               /MAXITER=50 /CONV=.0001 /PRIN=DEF /PLOT=NDIM(1,MAX) QUANT(5) OBJECT(pay)(8) DISCRIM.
             PRINCALS brand(3) shop(4) pay price(4) /ANAL=brand(SNOM) shop(MNOM) pay(ORDI) price(NUM)
               /DIM=3 /PLOT=ALL(6) NDIM(ALL,MAX).
             OVERALS brand shop pay price(4) /ANAL=brand(SNOM) shop(MNOM) pay price(NUM) /SET=2(2,2)
               /INIT=RANDOM /PLOT=DEFAULT(4) TRANS QUANT CENTROID(brand) /PRINT=NONE."
        ),
    )
    .unwrap();
    assert!(t.iter().any(|t| t.title == "Summary of Analysis"));
    assert!(!engine.take_charts().is_empty());
    run(
        &mut engine,
        "ANACOR TABLE=brand(1,3) BY shop(1,4) /VARIANCES=SINGULAR ROWS COLUMNS
           /PRINT=TABLE PROFILES PERMUTATION /PLOT=NDIM(ALL,MAX) JOINT(4) TRROWS TRCOLUMNS
           /MATRIX OUT=SCORE(*).",
    )
    .unwrap();
    let m = engine.dataset().unwrap();
    // three rows and four columns of scores
    assert_eq!(m.case_count(), 7);
    assert!(m.variable_index("ROWTYPE_").is_some());
}
