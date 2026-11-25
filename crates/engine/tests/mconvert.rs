//! MCONVERT on invented matrix materials: the covariance and correlation matrices of thirty
//! invented cases of three variables, computed in R (cov, cor, sqrt of the variances), converted
//! both ways, with record types left alone, appended, split into sets and written to a dataset.

use statscraft_data::Values;
use statscraft_engine::Engine;
use statscraft_syntax::parse;

const COV: [[f64; 3]; 3] = [
    [2.11279092, 1.273353103, -2.486318276],
    [1.273353103, 2.669292414, -1.463564138],
    [-2.486318276, -1.463564138, 72.18180931],
];
const SD: [f64; 3] = [1.453544261, 1.633796932, 8.495987836];
const CORR: [[f64; 3]; 3] = [
    [1.0, 0.5361946967, -0.2013328214],
    [0.5361946967, 1.0, -0.1054386414],
    [-0.2013328214, -0.1054386414, 1.0],
];
// the first fifteen cases
const COV2: [[f64; 3]; 3] = [
    [3.357309524, 1.796152381, -5.19122381],
    [1.796152381, 3.054680952, -2.568888095],
    [-5.19122381, -2.568888095, 90.0844381],
];
const CORR2: [[f64; 3]; 3] = [
    [1.0, 0.560873032, -0.2985033262],
    [0.560873032, 1.0, -0.1548593281],
    [-0.2985033262, -0.1548593281, 1.0],
];
const SD2: [f64; 3] = [1.832296243, 1.747764559, 9.491282216];

fn lines(kind: &str, m: &[[f64; 3]; 3]) -> String {
    m.iter()
        .map(|r| format!("{kind} {} {} {}\n", r[0], r[1], r[2]))
        .collect()
}

fn vector(kind: &str, v: &[f64; 3]) -> String {
    format!("{kind} {} {} {}\n", v[0], v[1], v[2])
}

fn matrix_data(body: &str) -> String {
    format!("MATRIX DATA VARIABLES=ROWTYPE_ a b c /FORMAT=FULL.\nBEGIN DATA\n{body}END DATA.\n")
}

fn run_engine(source: &str) -> Engine {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    for command in &commands {
        engine
            .execute(&command.command)
            .unwrap_or_else(|e| panic!("line {}: {e}", command.first_line));
    }
    engine
}

fn error(source: &str) -> String {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    for command in &commands {
        if let Err(e) = engine.execute(&command.command) {
            return e.to_string();
        }
    }
    panic!("no error");
}

type Row = (String, String, [Option<f64>; 3]);

fn records(engine: &Engine) -> Vec<Row> {
    records_of(engine.dataset().unwrap())
}

fn records_of(data: &statscraft_data::Dataset) -> Vec<Row> {
    let text = |name: &str| -> Vec<String> {
        let variable = data.variable(name).unwrap();
        let Values::String(v) = variable.values() else {
            panic!("{name} is not a string")
        };
        (0..data.case_count())
            .map(|i| v.get(i).trim_end().to_string())
            .collect()
    };
    let num = |name: &str, i: usize| match data.variable(name).unwrap().values() {
        Values::Numeric(v) => v.get(i),
        Values::String(_) => panic!("{name} is a string"),
    };
    let (kinds, names) = (text("ROWTYPE_"), text("VARNAME_"));
    (0..data.case_count())
        .map(|i| {
            (
                kinds[i].clone(),
                names[i].clone(),
                [num("a", i), num("b", i), num("c", i)],
            )
        })
        .collect()
}

fn close(row: &Row, kind: &str, name: &str, want: &[f64; 3]) {
    assert_eq!((row.0.as_str(), row.1.as_str()), (kind, name), "{row:?}");
    for (g, w) in row.2.iter().zip(want) {
        let g = g.unwrap_or_else(|| panic!("{row:?} has a missing value"));
        assert!(
            (g - w).abs() <= 1e-8 * w.abs().max(1.0),
            "{kind} {name}: {g} vs {w}"
        );
    }
}

fn matrix_rows(rows: &[Row], kind: &str, want: &[[f64; 3]; 3]) {
    assert_eq!(rows.len(), 3, "{rows:?}");
    for (i, (row, name)) in rows.iter().zip(["a", "b", "c"]).enumerate() {
        close(row, kind, name, &want[i]);
    }
}

#[test]
fn a_covariance_matrix_becomes_correlations_and_standard_deviations() {
    let e = run_engine(&format!("{}MCONVERT.\n", matrix_data(&lines("COV", &COV))));
    let rows = records(&e);
    assert_eq!(rows.len(), 4);
    close(&rows[0], "STDDEV", "", &SD);
    matrix_rows(&rows[1..], "CORR", &CORR);
}

#[test]
fn correlations_with_standard_deviations_become_a_covariance_matrix() {
    let body = format!("{}{}", vector("STDDEV", &SD), lines("CORR", &CORR));
    let e = run_engine(&format!("{}MCONVERT.\n", matrix_data(&body)));
    let rows = records(&e);
    assert_eq!(rows.len(), 4);
    close(&rows[0], "STDDEV", "", &SD);
    matrix_rows(&rows[1..], "COV", &COV);
}

#[test]
fn append_keeps_the_original_set_and_other_records_stay_in_place() {
    let body = format!(
        "{}{}",
        vector("MEAN", &[10.0, 11.0, 12.0]),
        lines("COV", &COV)
    );
    // REPLACE: the records other than the matrix stay where they were
    let e = run_engine(&format!("{}MCONVERT /REPLACE.\n", matrix_data(&body)));
    let rows = records(&e);
    assert_eq!(rows.len(), 5);
    close(&rows[0], "MEAN", "", &[10.0, 11.0, 12.0]);
    close(&rows[1], "STDDEV", "", &SD);
    matrix_rows(&rows[2..], "CORR", &CORR);
    // APPEND: the original records, then the generated STDDEV and the converted matrix
    let e = run_engine(&format!("{}MCONVERT /APPEND.\n", matrix_data(&body)));
    let rows = records(&e);
    assert_eq!(rows.len(), 8);
    close(&rows[0], "MEAN", "", &[10.0, 11.0, 12.0]);
    matrix_rows(&rows[1..4], "COV", &COV);
    close(&rows[4], "STDDEV", "", &SD);
    matrix_rows(&rows[5..], "CORR", &CORR);
}

#[test]
fn a_standard_deviation_record_the_set_has_is_kept_and_none_is_added() {
    let body = format!("{}{}", vector("STDDEV", &SD), lines("COV", &COV));
    let e = run_engine(&format!("{}MCONVERT.\n", matrix_data(&body)));
    let rows = records(&e);
    assert_eq!(rows.len(), 4);
    close(&rows[0], "STDDEV", "", &SD);
    matrix_rows(&rows[1..], "CORR", &CORR);
}

#[test]
fn there_and_back_gives_the_original_matrix() {
    let e = run_engine(&format!(
        "{}MCONVERT.\nMCONVERT.\n",
        matrix_data(&lines("COV", &COV))
    ));
    let rows = records(&e);
    // COV, then its STDDEV and CORR, then back: the STDDEV stays and the COV returns
    assert_eq!(rows.len(), 4);
    close(&rows[0], "STDDEV", "", &SD);
    matrix_rows(&rows[1..], "COV", &COV);
}

#[test]
fn every_set_of_a_split_file_is_converted_on_its_own() {
    let source = format!(
        "MATRIX DATA VARIABLES=g ROWTYPE_ a b c /SPLIT=g /FORMAT=FULL.\nBEGIN DATA\n{}{}END DATA.\nMCONVERT.\n",
        lines("1 COV", &COV),
        lines("2 COV", &COV2)
    );
    let e = run_engine(&source);
    let data = e.dataset().unwrap();
    assert_eq!(
        data.split_file().map(|s| s.variables.clone()),
        Some(vec!["g".to_string()])
    );
    let rows = records(&e);
    assert_eq!(rows.len(), 8);
    close(&rows[0], "STDDEV", "", &SD);
    matrix_rows(&rows[1..4], "CORR", &CORR);
    close(&rows[4], "STDDEV", "", &SD2);
    matrix_rows(&rows[5..], "CORR", &CORR2);
    let Values::Numeric(g) = data.variable("g").unwrap().values() else {
        panic!("g is not numeric")
    };
    let groups: Vec<Option<f64>> = (0..8).map(|i| g.get(i)).collect();
    assert_eq!(
        groups,
        [Some(1.0); 4]
            .into_iter()
            .chain([Some(2.0); 4])
            .collect::<Vec<_>>()
    );
}

#[test]
fn out_writes_a_dataset_and_leaves_the_active_one() {
    let source = format!(
        "{}DATASET DECLARE converted.\nMCONVERT MATRIX=OUT(converted) /APPEND.\n",
        matrix_data(&lines("COV", &COV))
    );
    let e = run_engine(&source);
    // the active dataset is the original
    matrix_rows(&records(&e), "COV", &COV);
    let (named, _) = e.named_datasets();
    let (_, written) = named
        .iter()
        .find(|(n, _)| n == "converted")
        .expect("declared");
    let rows = records_of(written.as_ref().expect("written"));
    assert_eq!(rows.len(), 7);
    matrix_rows(&rows[..3], "COV", &COV);
    close(&rows[3], "STDDEV", "", &SD);
    matrix_rows(&rows[4..], "CORR", &CORR);
    // IN(*) names the active dataset as the default does
    let e = run_engine(&format!(
        "{}MCONVERT MATRIX=IN(*).\n",
        matrix_data(&lines("COV", &COV))
    ));
    assert_eq!(records(&e).len(), 4);
}

#[test]
fn mconvert_is_checked() {
    let data = |body: &str| matrix_data(body);
    assert!(
        error(&format!("{}MCONVERT.", data(&lines("CORR", &CORR))))
            .contains("needs a STDDEV record")
    );
    let both = format!("{}{}", lines("CORR", &CORR), lines("COV", &COV));
    assert!(error(&format!("{}MCONVERT.", data(&both))).contains("both CORR and COV"));
    // two records for three variables, from a file built by hand
    let short = "DATA LIST FREE /ROWTYPE_ (A8) VARNAME_ (A8) a b c.\nBEGIN DATA\nCOV a 1 .5 .2\nCOV b .5 1 .1\nEND DATA.\nMCONVERT.";
    assert!(error(short).contains("2 records for 3 variables"));
    assert!(
        error("DATA LIST FREE /a b.\nBEGIN DATA\n1 2\nEND DATA.\nMCONVERT.")
            .contains("needs the string variable ROWTYPE_")
    );
    for (text, wanted) in [
        ("MCONVERT /APPEND /REPLACE.", "cannot be given on the same"),
        ("MCONVERT MATRIX=IN(m) OUT(m).", "cannot name the same file"),
        ("MCONVERT /NOSUCH.", "expected"),
    ] {
        let (_, errors) = parse(&format!("{}{text}", data(&lines("COV", &COV))));
        assert!(
            errors.first().is_some_and(
                |e| e.message.contains(wanted) || e.expected.iter().any(|x| x.contains(wanted))
            ),
            "{text}: {errors:?}"
        );
    }
}
