//! ALSCAL with SHAPE=RECTANGULAR (unfolding): rows and columns of the data are two sets of
//! stimuli in one space. Exact cases, which need no outside program: distances between made-up
//! row points and column points are recovered, with the shape of both sets of points, at the
//! ratio and interval level, under row conditionality with rows on scales of their own, and
//! with one matrix or several. The last test unfolds noisy distances and compares the minimum
//! of S-stress with the one R finds (BFGS with the analytic gradient from 60 random starts per
//! dimension, the gradient at the minimum below 1e-8): S-stress is the root of one minus the
//! squared cosine of the squared distances and the squared data over the whole rectangle.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

fn run(data: &str, syntax: &str) -> Vec<Table> {
    let (commands, errors) = parse(&format!("{data}{syntax}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        match engine.execute(&command.command) {
            Ok(more) => tables.extend(more),
            Err(error) => panic!("line {}: {error}", command.first_line),
        }
    }
    tables
}

fn number(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        other => panic!("not a number: {other:?}"),
    }
}

fn table<'a>(tables: &'a [Table], title: &str) -> &'a Table {
    tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"))
}

fn final_sstress(tables: &[Table]) -> f64 {
    let rows = &table(tables, "Iteration History").rows;
    number(&rows[rows.len() - 1][1])
}

fn data_of(rows: &str) -> String {
    format!("DATA LIST FREE /c1 TO c6.\nBEGIN DATA\n{rows}\nEND DATA.\n")
}

/// Five row points and six column points in a plane, and the distances between the two sets.
const EXACT: &str = "3.605551275 7.071067812 5 6 2.236067977 5
1.414213562 4.123105626 5.656854249 2.236067977 2 2
3.16227766 3 2 6.403124237 4 2.828427125
3.605551275 1.414213562 6.08276253 4 5 2.236067977
2.236067977 4.472135955 2.236067977 5.830951895 2.236067977 3";

/// The distances between all eleven points: rows first, then columns.
const TRUTH: &str = "0 4.123105626 5.385164807 7.211102551 3.16227766 3.605551275 7.071067812 5 6 2.236067977 5
4.123105626 0 4.472135955 3.605551275 3.605551275 1.414213562 4.123105626 5.656854249 2.236067977 2 2
5.385164807 4.472135955 0 4.123105626 2.236067977 3.16227766 3 2 6.403124237 4 2.828427125
7.211102551 3.605551275 4.123105626 0 5.099019514 3.605551275 1.414213562 6.08276253 4 5 2.236067977
3.16227766 3.605551275 2.236067977 5.099019514 0 2.236067977 4.472135955 2.236067977 5.830951895 2.236067977 3
3.605551275 1.414213562 3.16227766 3.605551275 2.236067977 0 3.605551275 4.242640687 3.605551275 1.414213562 1.414213562
7.071067812 4.123105626 3 1.414213562 4.472135955 3.605551275 0 5 5.099019514 5 2.236067977
5 5.656854249 2 6.08276253 2.236067977 4.242640687 5 0 7.810249676 4.472135955 4.472135955
6 2.236067977 6.403124237 4 5.830951895 3.605551275 5.099019514 7.810249676 0 4.123105626 3.605551275
2.236067977 2 4 5 2.236067977 1.414213562 5 4.472135955 4.123105626 0 2.828427125
5 2 2.828427125 2.236067977 3 1.414213562 2.236067977 4.472135955 3.605551275 2.828427125 0";

/// The coordinates the tables show: the rows, then the columns.
fn points(tables: &[Table], suffix: &str) -> Vec<Vec<f64>> {
    ["Row", "Column"]
        .iter()
        .flat_map(|set| {
            table(tables, &format!("{set} Stimulus Coordinates{suffix}"))
                .rows
                .iter()
                .map(|r| r[1..].iter().map(number).collect::<Vec<f64>>())
                .collect::<Vec<_>>()
        })
        .collect()
}

/// How far the distances between all the points of the tables are from the true ones, after
/// the best common scale.
fn shape_error(tables: &[Table]) -> f64 {
    let x = points(tables, "");
    let truth: Vec<Vec<f64>> = TRUTH
        .lines()
        .map(|l| l.split_whitespace().map(|v| v.parse().unwrap()).collect())
        .collect();
    let (mut dd, mut dt) = (0.0, 0.0);
    let mut pairs = Vec::new();
    for i in 0..x.len() {
        for j in 0..i {
            let d: f64 = x[i]
                .iter()
                .zip(&x[j])
                .map(|(a, b)| (a - b).powi(2))
                .sum::<f64>()
                .sqrt();
            dd += d * d;
            dt += d * truth[i][j];
            pairs.push((d, truth[i][j]));
        }
    }
    let scale = dt / dd;
    pairs
        .iter()
        .map(|(d, t)| (d * scale - t).abs())
        .fold(0.0, f64::max)
}

const TIGHT: &str = "CONVERGE(0) ITER(1500) STRESSMIN(0)";

#[test]
fn exact_distances_between_two_sets_of_points_are_unfolded() {
    let data = data_of(EXACT);
    let tables = run(
        &data,
        &format!(
            "ALSCAL VARIABLES=c1 TO c6 /SHAPE=RECTANGULAR /LEVEL=RATIO /CRITERIA=DIMENS(2) {TIGHT}."
        ),
    );
    assert!(final_sstress(&tables) < 1e-5, "{}", final_sstress(&tables));
    assert!(shape_error(&tables) < 1e-3, "{}", shape_error(&tables));
    assert_eq!(table(&tables, "Row Stimulus Coordinates").rows.len(), 5);
    assert_eq!(table(&tables, "Column Stimulus Coordinates").rows.len(), 6);
}

/// The distances, row i multiplied by 1, 2.5, 0.6, 1.8, 3: every row on a scale of its own.
const SCALED_ROWS: &str = "3.605551275 7.071067812 5 6 2.236067977 5
3.535533906 10.30776406 14.14213562 5.590169944 5 5
1.897366596 1.8 1.2 3.841874542 2.4 1.697056275
6.489992296 2.545584412 10.94897255 7.2 9 4.024922359
6.708203932 13.41640786 6.708203932 17.49285568 6.708203932 9";

/// The squares of the distances doubled and 1.5 added, and the root taken: an affine
/// function of the squared distances.
const AFFINE: &str = "5.244044241 10.07472084 7.176350047 8.5732141 3.391164992 7.176350047
2.34520788 5.958187644 8.093207028 3.391164992 3.082207001 3.082207001
4.636809248 4.415880433 3.082207001 9.137833441 5.787918451 4.183300133
5.244044241 2.34520788 8.689073598 5.787918451 7.176350047 3.391164992
3.391164992 6.442049363 3.391164992 8.336666 3.391164992 4.415880433";

/// The distances raised to the power 1.7: an increasing function of the distances.
const POWER: &str = "8.848137225 27.80510271 15.42584657 21.03086452 3.927575151 15.42584657
1.802500925 11.11428638 19.02731384 3.927575151 3.249009585 3.249009585
7.079457844 6.47300784 3.249009585 23.48909467 10.55606329 5.856342784
8.848137225 1.802500925 21.52640295 10.55606329 15.42584657 3.927575151
3.927575151 12.76072931 3.927575151 20.03351149 3.927575151 6.47300784";

/// A second judge: the first axis counts 2, the second 0.5.
const WEIGHTED: &str = "4.472135955 7.90569415 3.535533906 8.485281374 2.915475947 6.041522987
1.58113883 3.16227766 6.32455532 2.915475947 2.828427125 1.414213562
2.549509757 4.242640687 2.828427125 6.670832032 2.828427125 3.16227766
4.472135955 1.58113883 8.514693183 2.828427125 6.041522987 2.915475947
2.915475947 5.830951895 2 7.38241153 2 4.242640687";

fn command(level: &str, extra: &str) -> String {
    format!(
        "ALSCAL VARIABLES=c1 TO c6 /SHAPE=RECTANGULAR /LEVEL={level} {extra} /CRITERIA=DIMENS(2) {TIGHT}."
    )
}

#[test]
fn an_affine_function_of_the_squared_distances_is_fitted_at_the_interval_level() {
    let tables = run(&data_of(AFFINE), &command("INTERVAL(1)", ""));
    assert!(final_sstress(&tables) < 1e-5, "{}", final_sstress(&tables));
    assert!(shape_error(&tables) < 1e-3, "{}", shape_error(&tables));
}

#[test]
fn an_increasing_function_of_the_distances_unfolds_at_the_ordinal_level() {
    let tables = run(&data_of(POWER), &command("ORDINAL", ""));
    assert!(final_sstress(&tables) < 0.02, "{}", final_sstress(&tables));
}

/// A row on a scale of its own, as in preference data: only the row-conditional fit is exact.
#[test]
fn rows_on_scales_of_their_own_are_unfolded_under_row_conditionality() {
    let data = data_of(SCALED_ROWS);
    let by_row = run(&data, &command("RATIO", "/CONDITION=ROW"));
    assert!(final_sstress(&by_row) < 1e-5, "{}", final_sstress(&by_row));
    assert!(shape_error(&by_row) < 1e-3, "{}", shape_error(&by_row));
    let by_matrix = run(&data, &command("RATIO", "/CONDITION=MATRIX"));
    assert!(
        final_sstress(&by_matrix) > 0.01,
        "{}",
        final_sstress(&by_matrix)
    );
}

#[test]
fn several_matrices_by_input_rows_share_the_stimuli() {
    // the same matrix twice: the fit of one
    let once = run(&data_of(EXACT), &command("RATIO", ""));
    let twice = run(
        &data_of(&format!("{EXACT}\n{EXACT}")),
        &command("RATIO", "/INPUT=ROWS(5)"),
    );
    assert!(final_sstress(&twice) < 1e-5);
    assert!(shape_error(&twice) < 1e-3);
    assert_eq!(
        table(&twice, "Stress and Squared Correlation (RSQ) in Distances")
            .rows
            .len(),
        3
    );
    assert!(final_sstress(&once) < 1e-5);
}

#[test]
fn individual_differences_unfold_weights_of_the_axes() {
    let data = data_of(&format!("{EXACT}\n{WEIGHTED}"));
    let tables = run(&data, &command("RATIO", "/INPUT=ROWS(5) /MODEL=INDSCAL"));
    assert!(final_sstress(&tables) < 1e-4, "{}", final_sstress(&tables));
    // the second judge weighs the axes 4 to 1 more than the first does
    let w = &table(&tables, "Subject Weights").rows;
    let ratio = |k: usize| number(&w[k][1]) / number(&w[k][2]);
    let relative = ratio(1) / ratio(0);
    assert!(
        (relative - 4.0).abs() < 0.01 || (relative - 0.25).abs() < 0.001,
        "{relative}"
    );
}

#[test]
fn a_missing_proximity_leaves_the_unfolding_exact() {
    let mut rows: Vec<String> = EXACT.lines().map(str::to_string).collect();
    rows[1] = rows[1].replacen(rows[1].split_whitespace().nth(2).unwrap(), ".", 1);
    let tables = run(&data_of(&rows.join("\n")), &command("RATIO", "/PRINT=DATA"));
    assert!(final_sstress(&tables) < 1e-4, "{}", final_sstress(&tables));
    assert!(shape_error(&tables) < 1e-2, "{}", shape_error(&tables));
    // the cell is empty in the data tables, not a number
    for title in ["Raw Data", "Optimally Scaled Data"] {
        let t = table(&tables, title);
        assert_eq!(t.rows[1][3], Cell::Empty, "{title}");
        assert!(
            matches!(t.rows[1][2], Cell::Number(n) if n.is_finite()),
            "{title}"
        );
    }
}

#[test]
fn the_data_and_the_options_are_shown_as_a_rectangle() {
    let tables = run(
        &data_of(EXACT),
        &format!(
            "ALSCAL VARIABLES=c1 TO c6 /SHAPE=RECTANGULAR /PRINT=DATA HEADER /PLOT /CRITERIA=DIMENS(2) {TIGHT}."
        ),
    );
    let raw = table(&tables, "Raw Data");
    assert_eq!((raw.rows.len(), raw.columns.len()), (5, 7));
    assert_eq!(raw.columns[1], "c1");
    assert_eq!(raw.rows[1][0], Cell::Text("Row 2".into()));
    assert!((number(&raw.rows[0][2]) - 7.071067812).abs() < 1e-8);
    let scaled = table(&tables, "Optimally Scaled Data");
    assert_eq!((scaled.rows.len(), scaled.columns.len()), (5, 7));
    let options = table(&tables, "Analysis Options");
    let row = |name: &str| {
        options
            .rows
            .iter()
            .find(|r| r[0] == Cell::Text(name.into()))
            .map(|r| r[1].clone())
    };
    assert_eq!(row("Shape"), Some(Cell::Text("Rectangular".into())));
    assert_eq!(
        row("Stimuli"),
        Some(Cell::Text("5 rows and 6 columns".into()))
    );
}

#[test]
fn the_outfile_names_rows_and_columns_and_starts_the_next_run_at_its_minimum() {
    let data = data_of(EXACT);
    let tables = run(
        &format!("{data}DATASET DECLARE conf.\n"),
        &format!(
            "ALSCAL VARIABLES=c1 TO c6 /SHAPE=RECTANGULAR /LEVEL=RATIO /CRITERIA=DIMENS(2) {TIGHT} /OUTFILE=conf.\n\
             ALSCAL VARIABLES=c1 TO c6 /SHAPE=RECTANGULAR /LEVEL=RATIO /FILE=conf /CRITERIA=DIMENS(2) CONVERGE(0) ITER(2) STRESSMIN(0)."
        ),
    );
    let histories: Vec<&Table> = tables
        .iter()
        .filter(|t| t.title == "Iteration History")
        .collect();
    assert_eq!(histories.len(), 2);
    // the second run starts where the first stopped
    assert!(number(&histories[1].rows[0][1]) < 1e-5);
}

#[test]
fn the_rectangle_is_checked() {
    let data = data_of(EXACT);
    let (_, errors) = parse(&format!(
        "{data}ALSCAL VARIABLES=c1 TO c6 /SHAPE=RECTANGULAR /MATRIX=IN(*)."
    ));
    assert!(
        errors
            .iter()
            .any(|e| e.message.contains("MATRIX=IN holds square")),
        "{errors:?}"
    );
    let (_, errors) = parse(&format!("{data}ALSCAL VARIABLES=c1 TO c6 /FILE=x COLCONF."));
    assert!(
        errors.iter().any(|e| e
            .message
            .contains("COLCONF of FILE needs SHAPE=RECTANGULAR")),
        "{errors:?}"
    );
    for (syntax, wanted) in [
        (
            "ALSCAL VARIABLES=c1 TO c3 /SHAPE=RECTANGULAR.",
            "at least 4 rows and 4 columns",
        ),
        (
            "ALSCAL VARIABLES=c1 TO c6 /SHAPE=RECTANGULAR /INPUT=ROWS(4).",
            "not a multiple of INPUT=ROWS(4)",
        ),
    ] {
        let (commands, errors) = parse(&format!("{data}{syntax}"));
        assert!(errors.is_empty(), "{errors:?}");
        let mut engine = Engine::default();
        let mut message = None;
        for c in &commands {
            if let Err(e) = engine.execute(&c.command) {
                message = Some(e.to_string());
            }
        }
        assert!(message.expect("an error").contains(wanted), "{syntax}");
    }
}

/// The distances of `EXACT` with multiplicative noise (R, seed 2611, lognormal with sigma 0.12),
/// rounded to two decimals.
const NOISY: &str = "4.49 8.01 4.73 7.99 2.6 4.26
1.27 3.93 4.47 2.21 1.71 1.75
3.67 2.78 2.42 6.37 3.57 2.93
4.25 1.22 6.4 3.25 3.47 2.55
2.04 4.14 2.21 4.72 2.21 3.52";

#[test]
fn noisy_distances_reach_the_minimum_r_finds() {
    let data = data_of(NOISY);
    // dimensions and the root of the least 1 - cos^2 that R found; most of its random starts end
    // in worse local minima, the engine's start from classical scaling in the best one
    for (dimensions, minimum) in [
        (1, 0.324545972888),
        (2, 0.144438313369),
        (3, 0.079737063734),
    ] {
        let tables = run(
            &data,
            &format!(
                "ALSCAL VARIABLES=c1 TO c6 /SHAPE=RECTANGULAR /LEVEL=RATIO /CRITERIA=DIMENS({dimensions}) {TIGHT}."
            ),
        );
        let got = final_sstress(&tables);
        assert!(
            (got - minimum).abs() < 1e-6,
            "{dimensions} dimensions: {got} against {minimum}"
        );
    }
}
