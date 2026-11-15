//! MULTIPLE CORRESPONDENCE on synthetic data. The expected numbers come from independent
//! computations: a general statistics package's correspondence analysis of the indicator
//! matrix for the data without missing values (principal inertias, standard object scores,
//! category centroids, masses and contributions), and a separate script that runs plain
//! alternating least squares on the homogeneity loss for passive missing values, case weights
//! and variable weights. Signs follow our rule: in every dimension the object with the largest
//! absolute score is positive.

use statscraft_data::Values;
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

/// Fifteen households: pet (1 dog, 2 cat, 3 fish), garden (1 no, 2 yes) and commute (1 foot,
/// 2 bus, 3 car).
const DATA: &str = "DATA LIST FREE / pet garden commute.
BEGIN DATA
1 1 1
1 1 2
1 2 1
2 2 3
2 1 3
2 2 2
3 1 1
3 2 3
3 2 2
1 1 1
2 2 3
3 1 2
1 2 3
2 1 1
3 2 1
END DATA.
VALUE LABELS pet 1 'dog' 2 'cat' 3 'fish'.
";

/// The same households with three passive gaps and a case weight.
const GAPS: &str = "DATA LIST FREE / pet garden commute wt.
BEGIN DATA
1 1 1 1
1 1 2 2
1 0 1 1
2 2 3 1
2 1 3 3
2 2 . 1
3 1 1 1
3 2 3 2
3 2 2 1
1 1 1 1
2 2 3 1
3 1 2 2
1 2 3 1
2 1 1 1
0 2 1 1
END DATA.
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
const INERTIA: &str = "Variance Accounted For / Inertia";
const D1: &str = "Dimension 1";
const D2: &str = "Dimension 2";

#[test]
fn no_missing_values_match_the_indicator_matrix_analysis() {
    let t = tables(
        "MULTIPLE CORRESPONDENCE pet garden commute /ANALYSIS=pet garden commute
           /PRINT=DESCRIP DISCRIM QUANT OBJECT /PLOT=NONE.",
    );
    check(&t, "Model Summary", "1", INERTIA, 0.5478916, 1e-6);
    check(&t, "Model Summary", "2", INERTIA, 0.4074896, 1e-6);
    check(&t, "Model Summary", "1", EIGEN, 3.0 * 0.5478916, 3e-6);
    // alpha = m (e - 1) / ((m - 1) e)
    let e1 = 3.0 * 0.5478916;
    check(
        &t,
        "Model Summary",
        "1",
        "Cronbach's Alpha",
        3.0 * (e1 - 1.0) / (2.0 * e1),
        1e-5,
    );
    check(
        &t,
        "Model Summary",
        "Mean",
        EIGEN,
        1.5 * (0.5478916 + 0.4074896),
        3e-6,
    );
    check(
        &t,
        "Model Summary",
        "1",
        "Variance Accounted For / % of Variance",
        54.78916,
        1e-4,
    );
    // the discrimination measures add up to the eigenvalue
    let d: f64 = ["pet", "garden", "commute"]
        .iter()
        .map(|v| value(&t, "Discrimination Measures", v, D1))
        .sum();
    assert!((d - value(&t, "Model Summary", "1", EIGEN)).abs() < 1e-9);
    check(
        &t,
        "Discrimination Measures",
        "Active Total",
        D2,
        3.0 * 0.4074896,
        3e-6,
    );
    // standard object scores
    check(&t, "Object Scores", "1", D1, -1.4341622, 1e-6);
    check(&t, "Object Scores", "1", D2, -0.9915292, 1e-6);
    check(&t, "Object Scores", "4", D1, 1.6072653, 1e-6);
    check(&t, "Object Scores", "15", D2, 0.7306383, 1e-6);
    // centroids, masses, inertias and contributions of the categories
    let q = "Quantifications: pet";
    check(
        &t,
        q,
        "dog",
        "Centroid Coordinates / Dimension 1",
        -0.7698041,
        1e-6,
    );
    check(
        &t,
        q,
        "fish",
        "Centroid Coordinates / Dimension 2",
        1.1349453,
        1e-6,
    );
    check(&t, q, "dog", "Mass", 1.0 / 9.0, 1e-12);
    check(&t, q, "dog", "Inertia", 2.0 / 9.0, 1e-12);
    let to_dimension = "Contribution of Point to Inertia of Dimension / Dimension 1";
    check(&t, q, "dog", to_dimension, 0.120177548, 1e-6);
    check(
        &t,
        "Quantifications: commute",
        "3",
        "Contribution of Point to Inertia of Dimension / Dimension 2",
        0.06800536,
        1e-6,
    );
    // mass times squared principal coordinate over the category's inertia
    let of_point = "Contribution of Dimension to Inertia of Point / Dimension 1";
    check(
        &t,
        q,
        "dog",
        of_point,
        (1.0 / 9.0) * 0.7698041f64.powi(2) / (2.0 / 9.0),
        1e-6,
    );
    // object 1: inertia (1/3)(1/5 + 1/7 + 1/6) - 1/15
    let o = "Object Contributions";
    check(&t, o, "1", "Mass", 1.0 / 15.0, 1e-12);
    check(&t, o, "1", "Inertia", 0.1031746, 1e-6);
    check(
        &t,
        o,
        "1",
        "Contribution of Point to Inertia of Dimension / Dimension 1",
        0.1371214,
        1e-6,
    );
    check(
        &t,
        o,
        "1",
        "Contribution of Dimension to Inertia of Point / Dimension 2",
        0.2588591,
        1e-6,
    );
    // descriptives
    check(&t, "pet", "cat", "Frequency", 5.0, 0.0);
    check(&t, "garden", "Total", "Frequency", 15.0, 0.0);
    assert_eq!(table(&t, "commute").caption.as_deref(), Some("Mode: 1"));
    let summary = table(&t, "Case Processing Summary");
    assert_eq!(summary.rows[0][1], Cell::Number(15.0));
}

#[test]
fn passive_gaps_case_weights_and_variable_weights() {
    let mut engine = Engine::default();
    let t = run(
        &mut engine,
        &format!(
            "{GAPS}MULTIPLE CORRESPONDENCE pet garden commute /ANALYSIS=pet garden commute
               /PRINT=DISCRIM /PLOT=NONE."
        ),
    );
    check(&t, "Model Summary", "1", EIGEN, 1.91136028, 1e-6);
    check(&t, "Model Summary", "2", EIGEN, 1.4517282, 1e-6);
    check(&t, "Discrimination Measures", "pet", D2, 0.71075608, 1e-6);
    check(&t, "Discrimination Measures", "garden", D1, 0.5843703, 1e-6);
    check(
        &t,
        "Discrimination Measures",
        "commute",
        D1,
        0.7197474,
        1e-6,
    );
    check(
        &t,
        "Case Processing Summary",
        "Active Cases with Missing Values",
        "N",
        3.0,
        0.0,
    );
    // the weight on ANALYSIS covers the whole list before it: pet and garden weigh 2
    let t = run(
        &mut engine,
        "WEIGHT BY wt.
         MULTIPLE CORRESPONDENCE pet garden commute /ANALYSIS=pet garden(WEIGHT=2) commute
           /PRINT=DISCRIM /PLOT=NONE.",
    );
    check(&t, "Model Summary", "1", EIGEN, 2.98845895, 1e-6);
    check(&t, "Model Summary", "2", EIGEN, 2.30076088, 1e-6);
    check(&t, "Model Summary", "1", INERTIA, 2.98845895 / 5.0, 1e-6);
    check(&t, "Discrimination Measures", "pet", D2, 0.96103909, 1e-6);
    check(
        &t,
        "Discrimination Measures",
        "garden",
        D1,
        0.52652414,
        1e-6,
    );
    check(
        &t,
        "Discrimination Measures",
        "commute",
        D2,
        0.31344847,
        1e-6,
    );
}

#[test]
fn normalizations_spread_the_singular_values() {
    let base = tables(
        "MULTIPLE CORRESPONDENCE pet garden commute /ANALYSIS=pet garden commute
           /PRINT=QUANT(pet) OBJECT /PLOT=NONE.",
    );
    let sigma1 = 0.5478916f64.sqrt();
    let x = value(&base, "Object Scores", "2", D1);
    let y = value(
        &base,
        "Quantifications: pet",
        "cat",
        "Centroid Coordinates / Dimension 1",
    );
    for (keyword, xp, yp) in [
        ("OPRINCIPAL", 1.0, -1.0),
        ("SYMMETRICAL", 0.5, -0.5),
        ("INDEPENDENT", 1.0, 0.0),
        ("0.5", 0.75, -0.75),
        ("VPRINCIPAL", 0.0, 0.0),
    ] {
        let t = tables(&format!(
            "MULTIPLE CORRESPONDENCE pet garden commute /ANALYSIS=pet garden commute
               /NORMALIZATION={keyword} /PRINT=QUANT(pet) OBJECT /PLOT=NONE."
        ));
        check(&t, "Object Scores", "2", D1, x * sigma1.powf(xp), 1e-6);
        let q = "Quantifications: pet";
        check(
            &t,
            q,
            "cat",
            "Centroid Coordinates / Dimension 1",
            y * sigma1.powf(yp),
            1e-6,
        );
    }
}

#[test]
fn supplementary_objects_and_variables_follow_the_fit() {
    // case 16 copies case 4 and is supplementary; extra copies pet
    let source = DATA.replace("3 2 1\nEND", "3 2 1\n2 2 3\nEND");
    let mut engine = Engine::default();
    let t = run(
        &mut engine,
        &format!(
            "{source}COMPUTE extra = pet.
             MULTIPLE CORRESPONDENCE pet garden commute extra /ANALYSIS=pet garden commute extra
               /SUPPLEMENTARY=OBJECT(16) VARIABLE(extra) /PRINT=DISCRIM OBJECT QUANT(extra)
               /PLOT=NONE."
        ),
    );
    check(&t, "Model Summary", "1", INERTIA, 0.5478916, 1e-6);
    for d in [D1, D2] {
        let active = value(&t, "Object Scores", "4", d);
        check(&t, "Object Scores", "16", d, active, 1e-9);
        let discr = value(&t, "Discrimination Measures", "pet", d);
        check(
            &t,
            "Discrimination Measures",
            "extra (supplementary)",
            d,
            discr,
            1e-9,
        );
    }
    assert_eq!(
        table(&t, "Object Scores").caption.as_deref(),
        Some("Supplementary objects: 16.")
    );
    check(
        &t,
        "Quantifications: extra",
        "1",
        "Centroid Coordinates / Dimension 1",
        -0.7698041,
        1e-6,
    );
}

#[test]
fn a_start_or_a_fixed_configuration_from_a_dataset() {
    let mut engine = Engine::default();
    run(
        &mut engine,
        &format!(
            "{DATA}DATASET NAME households.
             DATASET DECLARE scores.
             MULTIPLE CORRESPONDENCE pet garden commute /ANALYSIS=pet garden commute
               /PLOT=NONE /OUTFILE=OBJECT(scores).
             DATASET DECLARE start.
             DATA LIST FREE / a b.
             BEGIN DATA
             1 0
             0 1
             2 1
             1 1
             0 0
             3 2
             1 2
             2 0
             0 3
             1 3
             2 2
             3 0
             0 2
             2 3
             3 3
             END DATA.
             DATASET NAME start.
             DATASET ACTIVATE households."
        ),
    );
    // iterations from a poor start reach the same solution
    let t = run(
        &mut engine,
        "MULTIPLE CORRESPONDENCE pet garden commute /ANALYSIS=pet garden commute
           /CONFIGURATION=('start') /MAXITER=5000 /CRITITER=1e-12 /PRINT=HISTORY /PLOT=NONE.",
    );
    check(&t, "Model Summary", "1", INERTIA, 0.5478916, 1e-5);
    check(&t, "Model Summary", "2", INERTIA, 0.4074896, 1e-5);
    let history = table(&t, "Iteration History");
    assert!(history.rows.len() > 3, "{:?}", history.rows.len());
    // the loss is the summed variable weights minus the fit
    let last = history.rows.last().unwrap();
    let (Cell::Number(fit), Cell::Number(loss)) = (&last[1], &last[3]) else {
        panic!()
    };
    assert!((fit + loss - 3.0).abs() < 1e-9);
    // the fitted object scores, fixed: the variables reach the same discrimination measures
    let t = run(
        &mut engine,
        "MULTIPLE CORRESPONDENCE pet garden commute /ANALYSIS=pet garden commute
           /CONFIGURATION=FIXED('scores') /PRINT=DISCRIM /PLOT=NONE.",
    );
    assert!(!titles(&t).contains(&"Model Summary"));
    let base = tables(
        "MULTIPLE CORRESPONDENCE pet garden commute /ANALYSIS=pet garden commute /PLOT=NONE.",
    );
    for v in ["pet", "garden", "commute"] {
        let expected = value(&base, "Discrimination Measures", v, D2);
        check(
            &t,
            "Discrimination Measures",
            &format!("{v} (supplementary)"),
            D2,
            expected,
            1e-9,
        );
    }
}

fn numbers(engine: &Engine, name: &str) -> Vec<Option<f64>> {
    let data = engine.dataset().unwrap();
    match data.variable(name).unwrap().values() {
        Values::Numeric(v) => v.iter().collect(),
        Values::String(_) => panic!("{name} is a string"),
    }
}

#[test]
fn save_adds_quantified_variables_and_object_scores() {
    let mut engine = Engine::default();
    let source = DATA.replace("3 2 1\nEND", "3 2 1\n2 9 3\nEND");
    let t = run(
        &mut engine,
        &format!(
            "{source}MISSING VALUES garden (9).
             MULTIPLE CORRESPONDENCE pet garden commute /ANALYSIS=pet garden commute
               /MISSING=garden(LISTWISE) /PRINT=OBJECT QUANT(pet) /PLOT=NONE
               /SAVE=TRDATA OBJECT(sco(1))."
        ),
    );
    check(
        &t,
        "Case Processing Summary",
        "Excluded Cases",
        "N",
        1.0,
        0.0,
    );
    let scores = numbers(&engine, "sco1_1");
    assert_eq!(scores.len(), 16);
    assert!((scores[0].unwrap() - value(&t, "Object Scores", "1", D1)).abs() < 1e-12);
    assert_eq!(scores[15], None);
    let pet = numbers(&engine, "TRA1_2_1");
    let cat = value(
        &t,
        "Quantifications: pet",
        "cat",
        "Centroid Coordinates / Dimension 2",
    );
    assert!((pet[3].unwrap() - cat).abs() < 1e-12);
    // a second run takes the next free set number
    run(
        &mut engine,
        "MULTIPLE CORRESPONDENCE pet garden commute /ANALYSIS=pet garden commute /PLOT=NONE
           /SAVE=TRDATA(q) OBJECT.",
    );
    let data = engine.dataset().unwrap();
    for name in ["q3_2_1", "OBSCO2_1"] {
        assert!(data.variable_index(name).is_some(), "{name}");
    }
    run(
        &mut engine,
        "MULTIPLE CORRESPONDENCE pet garden commute /ANALYSIS=pet garden commute /PLOT=NONE
           /SAVE=OBJECT(sco).",
    );
    assert!(engine.dataset().unwrap().variable_index("sco2_2").is_some());
}

#[test]
fn discretizing_strings_fractions_and_groups() {
    let t = run(
        &mut Engine::default(),
        "DATA LIST FREE / town (A5) km rent.
         BEGIN DATA
         north 1.5 400
         south 2.25 420
         east 3.5 500
         west 4.0 380
         north 5.5 610
         south 6.75 640
         east 7.0 700
         west 8.25 720
         north 9.5 380
         south 10.0 900
         END DATA.
         MULTIPLE CORRESPONDENCE town km rent /ANALYSIS=town km rent
           /DISCRETIZATION=rent(GROUPING NCAT=2 DISTR=UNIFORM)
           /PRINT=DESCRIP /PLOT=NONE.",
    );
    // strings by their sorted values
    check(&t, "town", "east", "Frequency", 2.0, 0.0);
    check(&t, "town", "north", "Frequency", 3.0, 0.0);
    // ten fractions cut at the optimal normal points for seven groups (+-0.2803, +-0.8744,
    // +-1.6108 on the z-scores): only the middle five groups are used
    let km = table(&t, "km");
    assert_eq!(km.rows.len(), 5 + 2, "{:?}", km.rows);
    check(&t, "km", "2", "Frequency", 2.0, 0.0);
    check(&t, "km", "4", "Frequency", 1.0, 0.0);
    check(&t, "km", "5", "Frequency", 3.0, 0.0);
    // two groups near five cases each
    check(&t, "rent", "1", "Frequency", 5.0, 0.0);
    check(&t, "rent", "2", "Frequency", 5.0, 0.0);
}

#[test]
fn active_imputation_and_its_extra_category() {
    let source = DATA.replace("3 2 1\nEND", "3 0 1\nEND");
    let t = run(
        &mut Engine::default(),
        &format!(
            "{source}MULTIPLE CORRESPONDENCE pet garden commute /ANALYSIS=pet garden commute
               /MISSING=garden(ACTIVE EXTRACAT) /PRINT=QUANT(garden) /PLOT=NONE."
        ),
    );
    check(
        &t,
        "Quantifications: garden",
        "Missing",
        "Frequency",
        1.0,
        0.0,
    );
    check(
        &t,
        "Case Processing Summary",
        "Active Cases with Missing Values",
        "N",
        0.0,
        0.0,
    );
    // a supplementary object's own code 3 stays apart from the extra category, which is 4
    let with_three = source.replace("3 0 1\nEND", "3 0 1\n1 3 1\nEND");
    let three = run(
        &mut Engine::default(),
        &format!(
            "{with_three}MULTIPLE CORRESPONDENCE pet garden commute /ANALYSIS=pet garden commute
               /MISSING=garden(ACTIVE EXTRACAT) /SUPPLEMENTARY=OBJECT(16) /PRINT=QUANT(garden)
               /PLOT=NONE."
        ),
    );
    let q = "Quantifications: garden";
    check(&three, q, "Missing", "Frequency", 1.0, 0.0);
    check(
        &three,
        q,
        "3 (supplementary objects)",
        "Frequency",
        1.0,
        0.0,
    );
    let t = run(
        &mut Engine::default(),
        &format!(
            "{source}MULTIPLE CORRESPONDENCE pet garden commute /ANALYSIS=pet garden commute
               /MISSING=garden(ACTIVE) /PRINT=QUANT(garden) /PLOT=NONE."
        ),
    );
    // garden has seven of each code among the others: the smaller mode, 1, is imputed
    check(&t, "Quantifications: garden", "1", "Frequency", 8.0, 0.0);
    check(&t, "Quantifications: garden", "2", "Frequency", 7.0, 0.0);
}

#[test]
fn charts_warnings_and_errors() {
    let mut engine = Engine::default();
    run(
        &mut engine,
        &format!(
            "{DATA}MULTIPLE CORRESPONDENCE pet garden commute /ANALYSIS=pet garden commute
               /DIMENSION=9."
        ),
    );
    assert!(engine.warnings().iter().any(|w| w.contains("at most 5")));
    let charts = engine.take_charts();
    let names: Vec<&str> = charts.iter().map(|c| c.title.as_str()).collect();
    assert_eq!(names, vec!["Object Points", "Discrimination Measures"]);
    run(
        &mut engine,
        "MULTIPLE CORRESPONDENCE pet garden commute /ANALYSIS=pet garden commute
           /PLOT=OBJECT(pet) CATEGORY(garden) JOINTCAT BIPLOT TRANS(pet(2)) RESID(commute).",
    );
    let names: Vec<String> = engine.take_charts().into_iter().map(|c| c.title).collect();
    assert_eq!(
        names,
        vec![
            "Object Points Labeled by pet",
            "Category Points: garden",
            "Joint Plot of Category Points",
            "Biplot",
            "Transformation: pet, Dimension 1",
            "Transformation: pet, Dimension 2",
            "Residuals: commute, Dimension 1",
        ]
    );
    assert!(
        run_error(&format!(
            "{DATA}SELECT IF $CASENUM < 3.
             MULTIPLE CORRESPONDENCE pet garden commute /ANALYSIS=pet garden commute."
        ))
        .contains("at least three")
    );
    assert!(
        run_error(&format!(
            "{DATA}MULTIPLE CORRESPONDENCE pet garden /ANALYSIS=pet commute."
        ))
        .contains("commute")
    );
    assert!(
        run_error(&format!(
            "{DATA}MULTIPLE CORRESPONDENCE pet garden commute /ANALYSIS=pet garden commute
               /SUPPLEMENTARY=VARIABLE(garden) /MISSING=garden(LISTWISE)."
        ))
        .contains("LISTWISE")
    );
}

#[test]
fn split_file_does_not_divide_the_analysis() {
    let t = tables(
        "SORT CASES BY garden.
         SPLIT FILE BY garden.
         MULTIPLE CORRESPONDENCE pet garden commute /ANALYSIS=pet garden commute /PLOT=NONE.",
    );
    check(&t, "Model Summary", "1", INERTIA, 0.5478916, 1e-6);
}
