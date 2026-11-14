//! PROXIMITIES on synthetic data. The expected numbers come from a general statistics
//! package used as a black box: its distance function (Euclidean, Manhattan, binary), its
//! column scaling and its correlation matrix, on the same values.

use statscraft_data::{Value, Values};
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

/// Seven shops; west misses sour, so six take part in the measures over the three tastes.
/// b1 to b4 are presence (1) and absence (0).
const DATA: &str = "DATA LIST FREE /shop (A6) sweet sour bitter b1 b2 b3 b4.
BEGIN DATA.
north 3.0 1.5 0.2 1 0 1 1
south 4.5 2.0 1.1 0 0 1 0
east  1.0 3.5 2.4 1 1 0 0
west  2.5 .   0.9 1 1 1 0
hill  5.0 0.5 1.8 0 1 1 1
lake  0.5 2.5 3.0 1 0 0 1
coast 3.5 3.0 0.4 0 1 0 1
END DATA.
";

fn run_on(engine: &mut Engine, source: &str) -> Vec<Table> {
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

fn run(syntax: &str) -> Vec<Table> {
    run_on(&mut Engine::default(), &format!("{DATA}{syntax}"))
}

fn number(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        other => panic!("not a number: {other:?}"),
    }
}

fn matrix(tables: &[Table]) -> (&Table, Vec<Vec<f64>>) {
    let table = tables
        .iter()
        .find(|t| t.title == "Proximity Matrix")
        .expect("a proximity matrix");
    let values = table
        .rows
        .iter()
        .map(|r| r[1..].iter().map(number).collect())
        .collect();
    (table, values)
}

fn close(got: &[Vec<f64>], want: &[&[f64]]) {
    assert_eq!(got.len(), want.len());
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        for (j, (a, b)) in g.iter().zip(w.iter()).enumerate() {
            assert!((a - b).abs() < 1e-9, "[{i}][{j}]: {a} against {b}");
        }
    }
}

const EUCLID: [&[f64]; 6] = [
    &[
        0.0,
        1.81934053987,
        3.58329457343,
        2.74954541697,
        3.88458491991,
        1.59373774505,
    ],
    &[
        1.81934053987,
        0.0,
        4.02367990775,
        1.72916164658,
        4.45645599103,
        1.57797338381,
    ],
    &[
        3.58329457343,
        4.02367990775,
        0.0,
        5.03587132481,
        1.26885775404,
        3.24037034920,
    ],
    &[
        2.74954541697,
        1.72916164658,
        5.03587132481,
        0.0,
        5.06853035899,
        3.23419232576,
    ],
    &[
        3.88458491991,
        4.45645599103,
        1.26885775404,
        5.06853035899,
        0.0,
        4.00124980475,
    ],
    &[
        1.59373774505,
        1.57797338381,
        3.24037034920,
        3.23419232576,
        4.00124980475,
        0.0,
    ],
];

#[test]
fn euclidean_distances_between_cases_leave_out_a_case_with_a_missing_value() {
    let tables = run("PROXIMITIES sweet sour bitter /ID=shop.");
    let summary = &tables[0];
    assert_eq!(summary.title, "Case Processing Summary");
    assert_eq!(number(&summary.rows[0][0]), 6.0);
    assert_eq!(number(&summary.rows[0][2]), 1.0);
    let (table, values) = matrix(&tables);
    close(&values, &EUCLID);
    assert_eq!(table.columns[1], "1:north");
    // hill is case 5: west, case 4, is left out
    assert_eq!(table.columns[4], "5:hill");
    assert!(table.caption.as_deref().unwrap().contains("dissimilarity"));
}

#[test]
fn z_scores_by_variable_then_block_distances() {
    let tables = run("PROXIMITIES sweet sour bitter /STANDARDIZE /MEASURE=BLOCK.");
    let (_, values) = matrix(&tables);
    close(
        &values,
        &[
            &[
                0.0,
                2.09115007279,
                4.92005050214,
                3.45577623913,
                4.80620415828,
                1.84173446342,
            ],
            &[
                2.09115007279,
                0.0,
                4.47001798545,
                2.29044626612,
                4.35617164159,
                2.10105580891,
            ],
            &[
                4.92005050214,
                4.47001798545,
                0.0,
                5.50407120401,
                1.73779385569,
                3.62535522408,
            ],
            &[
                3.45577623913,
                2.29044626612,
                5.50407120401,
                0.0,
                5.39022486016,
                4.39150207503,
            ],
            &[
                4.80620415828,
                4.35617164159,
                1.73779385569,
                5.39022486016,
                0.0,
                4.43732898000,
            ],
            &[
                1.84173446342,
                2.10105580891,
                3.62535522408,
                4.39150207503,
                4.43732898000,
                0.0,
            ],
        ],
    );
}

#[test]
fn correlations_between_variables() {
    let tables = run("PROXIMITIES sweet sour bitter /VIEW=VARIABLE /MEASURE=CORRELATION.");
    let (table, values) = matrix(&tables);
    assert_eq!(table.columns, ["Variable", "sweet", "sour", "bitter"]);
    close(
        &values,
        &[
            &[1.0, -0.675279830898, -0.599747780982],
            &[-0.675279830898, 1.0, 0.202174184225],
            &[-0.599747780982, 0.202174184225, 1.0],
        ],
    );
    assert!(
        table
            .caption
            .as_deref()
            .unwrap()
            .contains("a similarity matrix")
    );
}

#[test]
fn transformations_run_in_their_own_order_on_the_whole_matrix() {
    // RESCALE written first still comes after REVERSE
    let tables = run(
        "PROXIMITIES sweet sour bitter /STANDARDIZE=CASE RESCALE /MEASURE=RESCALE SEUCLID REVERSE.",
    );
    let (table, values) = matrix(&tables);
    close(
        &values[..1],
        &[&[
            1.0,
            0.982579743660,
            0.2999947775905,
            0.8692268485295,
            0.0760256660888,
            0.938687572971,
        ]],
    );
    assert_eq!(values[1][4], 0.0);
    assert!(
        table
            .caption
            .as_deref()
            .unwrap()
            .contains("a similarity matrix")
    );
}

#[test]
fn jaccard_between_cases() {
    let tables = run("PROXIMITIES b1 TO b4 /MEASURE=JACCARD /PRINT=PROXIMITIES.");
    let (_, values) = matrix(&tables);
    let third = 1.0 / 3.0;
    close(
        &values[..2],
        &[
            &[1.0, third, 0.25, 0.5, 0.5, 2.0 * third, 0.25],
            &[third, 1.0, 0.0, third, third, 0.0, 0.0],
        ],
    );
}

#[test]
fn print_none_keeps_only_the_summary() {
    let tables = run("PROXIMITIES sweet sour /PRINT=NONE.");
    assert_eq!(tables.len(), 1);
}

fn text(data: &statscraft_data::Dataset, name: &str, case: usize) -> String {
    match data.variable(name).unwrap().values() {
        Values::String(s) => s.get(case).trim().to_string(),
        Values::Numeric(v) => v.get(case).map_or(".".into(), |v| v.to_string()),
    }
}

fn value(data: &statscraft_data::Dataset, name: &str, case: usize) -> f64 {
    data.variable(name)
        .unwrap()
        .numeric()
        .unwrap()
        .get(case)
        .unwrap()
}

#[test]
fn matrix_out_replaces_the_active_dataset_and_reads_back() {
    let mut engine = Engine::default();
    run_on(
        &mut engine,
        &format!("{DATA}PROXIMITIES sweet sour bitter /ID=shop /PRINT=NONE /MATRIX=OUT(*)."),
    );
    let data = engine.dataset().unwrap();
    let names: Vec<&str> = data.variable_names().collect();
    assert_eq!(
        names,
        [
            "ROWTYPE_", "shop", "CASENO_", "VARNAME_", "VAR1", "VAR2", "VAR3", "VAR4", "VAR5",
            "VAR6"
        ]
    );
    assert_eq!(data.case_count(), 6);
    assert_eq!(text(data, "ROWTYPE_", 0), "PROX");
    assert_eq!(text(data, "shop", 3), "hill");
    assert_eq!(value(data, "CASENO_", 3), 5.0);
    assert_eq!(text(data, "VARNAME_", 2), "VAR3");
    assert!((value(data, "VAR2", 0) - EUCLID[0][1]).abs() < 1e-9);
    assert_eq!(data.variable("VAR4").unwrap().label(), Some("CASE 5"));
    let labels = data.variable("ROWTYPE_").unwrap().value_labels().to_vec();
    assert_eq!(
        labels,
        [(
            Value::String("PROX".into()),
            "Euclidean Distance DISSIMILARITY".into()
        )]
    );
    // the matrix read back: transformed only, labeled by the ID variable
    let tables = run_on(
        &mut engine,
        "PROXIMITIES /MATRIX=IN(*) /ID=shop /MEASURE=NONE REVERSE RESCALE.",
    );
    let (table, values) = matrix(&tables);
    assert_eq!(table.columns[4], "hill");
    let largest = 5.06853035899;
    assert!((values[0][1] - (largest - EUCLID[0][1]) / largest).abs() < 1e-9);
    assert!(
        table
            .caption
            .as_deref()
            .unwrap()
            .contains("a similarity matrix")
    );
    // CLUSTER reads it too: the closest pair is east and lake
    let tables = run_on(&mut engine, "CLUSTER /MATRIX=IN(*) /METHOD=SINGLE.");
    let schedule = tables
        .iter()
        .find(|t| t.title == "Agglomeration Schedule")
        .unwrap();
    assert!((number(&schedule.rows[0][3]) - EUCLID[2][4]).abs() < 1e-9);
}

#[test]
fn matrix_out_under_a_split_writes_one_matrix_per_group() {
    let mut engine = Engine::default();
    run_on(
        &mut engine,
        &format!(
            "{DATA}COMPUTE side = shop = 'east' | shop = 'west' | shop = 'lake' | shop = 'coast'.\n\
             SORT CASES BY side.\n\
             SPLIT FILE BY side.\n\
             PROXIMITIES sweet bitter /MATRIX=OUT(*)."
        ),
    );
    let data = engine.dataset().unwrap();
    let names: Vec<&str> = data.variable_names().collect();
    assert_eq!(&names[..4], ["side", "ROWTYPE_", "CASENO_", "VARNAME_"]);
    // three shops on side 0, four on side 1
    assert_eq!(data.case_count(), 7);
    assert_eq!(value(data, "side", 2), 0.0);
    assert_eq!(value(data, "side", 3), 1.0);
    // the case numbers of the sorted file: east is fourth
    assert_eq!(value(data, "CASENO_", 3), 4.0);
    // east and west: |1.0 - 2.5| and |2.4 - 0.9|
    assert!((value(data, "VAR2", 3) - (1.5f64.powi(2) * 2.0).sqrt()).abs() < 1e-12);
    // the second group has a fourth item, blank in the first
    assert!(
        data.variable("VAR4")
            .unwrap()
            .numeric()
            .unwrap()
            .get(0)
            .is_none()
    );
    assert_eq!(data.variable("VAR1").unwrap().label(), None);
}

#[test]
fn a_reversed_matrix_is_labeled_a_similarity_and_read_back_so() {
    let mut engine = Engine::default();
    run_on(
        &mut engine,
        &format!(
            "{DATA}PROXIMITIES sweet sour bitter /PRINT=NONE /MATRIX=OUT(*).\n\
             PROXIMITIES /MATRIX=IN(*) /MEASURE=NONE REVERSE /PRINT=NONE /MATRIX=OUT(*).\n"
        ),
    );
    let data = engine.dataset().unwrap();
    let labels = data.variable("ROWTYPE_").unwrap().value_labels().to_vec();
    assert_eq!(labels[0].1, "SIMILARITY");
    let tables = run_on(&mut engine, "PROXIMITIES /MATRIX=IN(*).");
    let (table, _) = matrix(&tables);
    assert!(
        table
            .caption
            .as_deref()
            .unwrap()
            .contains("a similarity matrix")
    );
}

#[test]
fn split_matrices_survive_temporary_selection_and_read_back_per_group() {
    let mut engine = Engine::default();
    run_on(
        &mut engine,
        &format!(
            "{DATA}COMPUTE side = shop = 'east' | shop = 'west' | shop = 'lake' | shop = 'coast'.\n\
             SORT CASES BY side.\n\
             SPLIT FILE BY side.\n\
             TEMPORARY.\n\
             SELECT IF shop <> 'north'.\n\
             PROXIMITIES sweet bitter /PRINT=NONE /MATRIX=OUT(*).\n"
        ),
    );
    let data = engine.dataset().unwrap();
    // south and hill on side 0, the four others on side 1
    assert_eq!(data.case_count(), 6);
    let sides: Vec<f64> = (0..6).map(|c| value(data, "side", c)).collect();
    assert_eq!(sides, [0.0, 0.0, 1.0, 1.0, 1.0, 1.0]);
    // each group reads back its own items
    let tables = run_on(
        &mut engine,
        "SPLIT FILE SEPARATE BY side.\nCLUSTER /MATRIX=IN(*) /PRINT=DISTANCE.",
    );
    let sizes: Vec<usize> = tables
        .iter()
        .filter(|t| t.title.starts_with("Proximity Matrix"))
        .map(|t| t.rows.len())
        .collect();
    assert_eq!(sizes, [2, 4]);
}

#[test]
fn an_upper_triangle_without_its_last_row_keeps_every_item() {
    let tables = run_on(
        &mut Engine::default(),
        "DATA LIST FREE /ROWTYPE_ (A8) VARNAME_ (A8) a b c.\nBEGIN DATA.\n\
         PROX a . 1 2\nPROX b . . 3\nEND DATA.\n\
         PROXIMITIES /MATRIX=IN(*).",
    );
    let table = tables
        .iter()
        .find(|t| t.title == "Proximity Matrix")
        .unwrap();
    assert_eq!(table.columns, ["Case", "a", "b", "c"]);
    // c, which has no row, against b; the diagonal the file leaves out is empty
    assert_eq!(number(&table.rows[2][2]), 3.0);
    assert_eq!(table.rows[2][3], Cell::Empty);
}

#[test]
fn a_split_variable_cannot_also_be_an_item() {
    let mut engine = Engine::default();
    let (commands, errors) = parse(&format!(
        "{DATA}COMPUTE side = sweet > 3.\nSORT CASES BY side.\nSPLIT FILE BY side.\n\
         PROXIMITIES sweet side /VIEW=VARIABLE /MATRIX=OUT(*).\n"
    ));
    assert!(errors.is_empty());
    let last = commands
        .iter()
        .map(|c| engine.execute(&c.command))
        .last()
        .unwrap();
    assert!(
        last.unwrap_err()
            .to_string()
            .contains("split variable side")
    );
}

#[test]
fn a_split_matrix_file_is_read_group_by_group_from_another_dataset() {
    let mut engine = Engine::default();
    let tables = run_on(
        &mut engine,
        &format!(
            "{DATA}DATASET NAME shops.\nDATASET DECLARE m.\n\
             COMPUTE side = shop = 'east' | shop = 'west' | shop = 'lake' | shop = 'coast'.\n\
             SORT CASES BY side.\n\
             SPLIT FILE SEPARATE BY side.\n\
             PROXIMITIES sweet bitter /PRINT=NONE /MATRIX=OUT(m).\n\
             CLUSTER /MATRIX=IN(m) /PRINT=DISTANCE.\n"
        ),
    );
    let sizes: Vec<usize> = tables
        .iter()
        .filter(|t| t.title.starts_with("Proximity Matrix"))
        .map(|t| t.rows.len())
        .collect();
    assert_eq!(sizes, [3, 4]);
    // ALL is the items of each group, three on side 0
    let tables = run_on(&mut engine, "CLUSTER ALL /MATRIX=IN(m) /PRINT=DISTANCE.");
    let sizes: Vec<usize> = tables
        .iter()
        .filter(|t| t.title.starts_with("Proximity Matrix"))
        .map(|t| t.rows.len())
        .collect();
    assert_eq!(sizes, [3, 4]);
    // naming an item twice is refused
    let (commands, _) = parse(
        "SPLIT FILE OFF.\nDATASET ACTIVATE m.\nSELECT IF side = 1.\nCLUSTER VAR1 VAR2 VAR1 /MATRIX=IN(*).",
    );
    let last = commands
        .iter()
        .map(|c| engine.execute(&c.command))
        .last()
        .unwrap();
    assert!(last.unwrap_err().to_string().contains("named twice"));
}

#[test]
fn all_with_matrix_input_means_every_item() {
    let mut engine = Engine::default();
    run_on(
        &mut engine,
        &format!(
            "{DATA}PROXIMITIES sweet sour bitter /VIEW=VARIABLE /PRINT=NONE /MATRIX=OUT(*).\n"
        ),
    );
    let tables = run_on(&mut engine, "PROXIMITIES ALL /MATRIX=IN(*).");
    let (table, _) = matrix(&tables);
    assert_eq!(table.columns, ["Case", "sweet", "sour", "bitter"]);
}

#[test]
fn a_split_variable_of_another_type_in_the_matrix_file_is_named() {
    let mut engine = Engine::default();
    let (commands, errors) = parse(
        "DATA LIST FREE /side (A1) ROWTYPE_ (A8) VARNAME_ (A8) a b c.\nBEGIN DATA.\n\
         1 PROX a 0 1 2\n1 PROX b 1 0 3\n1 PROX c 2 3 0\nEND DATA.\nDATASET NAME m.\n\
         DATA LIST FREE /side x.\nBEGIN DATA.\n1 4\n1 5\nEND DATA.\n\
         SPLIT FILE BY side.\nCLUSTER /MATRIX=IN(m).\n",
    );
    assert!(errors.is_empty(), "{errors:?}");
    let last = commands
        .iter()
        .map(|c| engine.execute(&c.command))
        .last()
        .unwrap();
    assert!(
        last.unwrap_err()
            .to_string()
            .contains("numeric in one file")
    );
}

#[test]
fn a_layered_split_shares_columns_by_place_and_the_matrix_keeps_the_split() {
    let mut engine = Engine::default();
    let tables = run_on(
        &mut engine,
        &format!(
            "{DATA}COMPUTE side = shop = 'east' | shop = 'west' | shop = 'lake' | shop = 'coast'.\n\
             SORT CASES BY side.\nSPLIT FILE BY side.\n\
             PROXIMITIES sweet bitter /MATRIX=OUT(*).\n"
        ),
    );
    let table = tables
        .iter()
        .find(|t| t.title == "Proximity Matrix")
        .unwrap();
    // the side and case columns, then places 1 to 4
    assert_eq!(&table.columns[2..], ["1", "2", "3", "4"]);
    // the matrix replaces the data with the split kept, so it reads back group by group
    let tables = run_on(&mut engine, "CLUSTER /MATRIX=IN(*).");
    assert!(
        tables
            .iter()
            .any(|t| t.title.starts_with("Agglomeration Schedule"))
    );
}
