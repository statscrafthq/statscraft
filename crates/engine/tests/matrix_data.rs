//! MATRIX DATA on the manual's layouts with synthetic numbers: the matrix file it builds, value
//! by value, and FACTOR and CLUSTER reading it with MATRIX=IN(*). The expected results follow
//! from the rules: a triangle is mirrored, a missing correlation diagonal is 1, N=n and
//! N_SCALAR repeat their value, a correlation matrix with every r = .5 has the eigenvalues
//! 1 + 2r = 2 and 1 - r = .5 (twice).

use statscraft_data::{Dataset, Values};
use statscraft_engine::session::Session;
use statscraft_engine::{Cell, Table};

fn run(source: &str) -> (Session, Vec<Table>, Vec<String>) {
    let mut session = Session::default();
    let result = session
        .run(source, None)
        .unwrap_or_else(|e| panic!("{source}\n{e:?}"));
    let tables = result.commands.iter().flat_map(|c| c.tables.clone());
    let warnings = result.commands.iter().flat_map(|c| c.warnings.clone());
    let (tables, warnings) = (tables.collect(), warnings.collect());
    (session, tables, warnings)
}

fn fails(source: &str) -> String {
    let errors = Session::default().run(source, None).unwrap_err();
    errors
        .iter()
        .map(|e| e.text())
        .collect::<Vec<_>>()
        .join(" | ")
}

/// Every case as text: strings trimmed, numbers as written, . for missing.
fn cases(data: &Dataset) -> Vec<String> {
    (0..data.case_count())
        .map(|case| {
            let fields: Vec<String> = data
                .variables()
                .map(|v| match v.values() {
                    Values::Numeric(x) => x.get(case).map_or(".".into(), |x| x.to_string()),
                    Values::String(s) => s.get(case).trim_end().to_string(),
                })
                .collect();
            fields.join(" ")
        })
        .collect()
}

fn names(data: &Dataset) -> Vec<String> {
    data.variables()
        .map(|v| format!("{} {}", v.name(), v.format()))
        .collect()
}

fn find<'a>(tables: &'a [Table], title: &str) -> &'a Table {
    tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"))
}

fn number(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        other => panic!("not a number: {other:?}"),
    }
}

#[test]
fn record_types_in_the_data() {
    let (session, _, _) = run("MATRIX DATA VARIABLES=ROWTYPE_ V1 TO V3.
[removed]
");
    let data = session.dataset().unwrap();
    assert_eq!(
        names(data),
        [
            "ROWTYPE_ A8",
            "VARNAME_ A8",
            "V1 F10.4",
            "V2 F10.4",
            "V3 F10.4"
        ]
    );
    assert_eq!(
        cases(data),
        [
            "MEAN  5 4 3",
            "STDDEV  3 2 1",
            "N  9 9 9",
            "CORR V1 1 0.6 0.7",
            "CORR V2 0.6 1 0.8",
            "CORR V3 0.7 0.8 1",
        ]
    );
}

#[test]
fn shapes_and_diagonals_give_the_same_matrix() {
    let full = "CORR 1 .6 .7\nCORR .6 1 .8\nCORR .7 .8 1";
    for (format, rows) in [
        ("FULL", full),
        ("UPPER", "CORR 1 .6 .7\nCORR 1 .8\nCORR 1"),
        ("UPPER NODIAGONAL", "CORR .6 .7\nCORR .8"),
        ("LOWER NODIAGONAL", "CORR .6\nCORR .7 .8"),
        ("LOWER DIAGONAL", "CORR 1\nCORR .6 1\nCORR .7 .8 1"),
    ] {
        let (session, _, _) = run(&format!(
            "MATRIX DATA VARIABLES=ROWTYPE_ V1 TO V3 /FORMAT={format}.\nBEGIN DATA\nN 9 9 9\n{rows}\nEND DATA.\n"
        ));
        assert_eq!(
            cases(session.dataset().unwrap()),
            [
                "N  9 9 9",
                "CORR V1 1 0.6 0.7",
                "CORR V2 0.6 1 0.8",
                "CORR V3 0.7 0.8 1"
            ],
            "{format}"
        );
    }
    // the diagonal of a covariance matrix left out is missing
    let (session, _, _) = run("MATRIX DATA VARIABLES=ROWTYPE_ a b /FORMAT=NODIAGONAL.
BEGIN DATA
COV 2.5
END DATA.
");
    assert_eq!(
        cases(session.dataset().unwrap()),
        ["COV a . 2.5", "COV b 2.5 ."]
    );
}

#[test]
fn contents_describe_the_records() {
    // values on several records, + and - as separators, D exponents and N=
    let (session, _, warnings) = run(
        [removed]
",
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(
        cases(session.dataset().unwrap()),
        [
            "MEAN  5 4 3",
            "STDDEV  3 2 1",
            "N  9 9 9",
            "CORR V1 1 0.6 0.7",
            "CORR V2 0.6 1 0.8",
            "CORR V3 0.7 0.8 1",
        ]
    );
    let (session, _, _) = run(
        [removed]
",
    );
    let data = session.dataset().unwrap();
    assert_eq!(cases(data)[0], "N  99 99 99");
    assert_eq!(cases(data)[2], "STDDEV  3 4 5");
    // FREE: rows need not start a new record
    let (session, _, _) = run("MATRIX DATA VARIABLES=a b /FORMAT=FREE /CONTENTS=MEAN CORR.
BEGIN DATA
1.5 2.5E0 1 -.25D0 1
END DATA.
");
    assert_eq!(
        cases(session.dataset().unwrap()),
        ["MEAN  1.5 2.5", "CORR a 1 -0.25", "CORR b -0.25 1"]
    );
}

#[test]
fn within_cells_records_and_factors() {
    let expected = [
        "MEAN 1  5 4 3",
        "STDDEV 1  3 2 1",
        "N 1  9 9 9",
        "MEAN 2  4 5 6",
        "STDDEV 2  6 5 4",
        "N 2  9 9 9",
        "CORR . VAR1 1 0.6 0.7",
        "CORR . VAR2 0.6 1 0.8",
        "CORR . VAR3 0.7 0.8 1",
    ];
    [removed]
");
    let data = session.dataset().unwrap();
    assert_eq!(
        names(data),
        [
            "ROWTYPE_ A8",
            "F1 F4.0",
            "VARNAME_ A8",
            "VAR1 F10.4",
            "VAR2 F10.4",
            "VAR3 F10.4"
        ]
    );
    assert_eq!(cases(data), expected);
    // the same records with ROWTYPE_ in the data, CELLS ignored with a warning
    let (session, _, warnings) = run(
        "MATRIX DATA VARIABLES=ROWTYPE_ F1 VAR1 TO VAR3 /FACTORS=F1 /CELLS=2.
BEGIN DATA
MEAN 1 5 4 3
SD   1 3 2 1
N    1 9 9 9
MEAN 2 4 5 6
SD   2 6 5 4
N    2 9 9 9
CORR . 1
CORR . .6 1
CORR . .7 .8 1
END DATA.
",
    );
    assert_eq!(cases(session.dataset().unwrap()), expected);
    assert!(warnings[0].contains("CELLS is ignored"), "{warnings:?}");
    // each type grouped across the cells
    [removed]
");
    let data = session.dataset().unwrap();
    assert_eq!(cases(data)[1], "MEAN 2  4 5 6");
    assert_eq!(cases(data)[2], "STDDEV 1  3 2 1");
}

#[test]
fn split_files() {
    let (session, _, _) = run(
        [removed]
",
    );
    let data = session.dataset().unwrap();
    assert_eq!(names(data)[0], "SPL F4.0");
    let cases = cases(data);
    assert_eq!(cases.len(), 12);
    assert_eq!(cases[0], "1 MEAN  5 4 3");
    assert_eq!(cases[11], "2 CORR V3 0.3 0.2 1");
    assert_eq!(
        data.split_file().map(|s| s.variables.clone()),
        Some(vec!["SPL".to_string()])
    );
    let (session, _, _) = run("MATRIX DATA VARIABLES=S1 ROWTYPE_ V1 TO V2 /SPLIT=S1.
BEGIN DATA
0 MEAN 5 4
0 CORR 1
0 CORR .6 1
1 MEAN 9 8
1 CORR 1
1 CORR .4 1
END DATA.
");
    assert_eq!(
        cases_of(&session)[3..],
        ["1 MEAN  9 8", "1 CORR V1 1 0.4", "1 CORR V2 0.4 1"]
    );
}

fn cases_of(session: &Session) -> Vec<String> {
    cases(session.dataset().unwrap())
}

#[test]
fn values_left_on_a_record_and_errors() {
    // an upper triangle in the default lower format: the extra values are ignored
    let (session, _, warnings) = run("MATRIX DATA VARIABLES=ROWTYPE_ a b.
BEGIN DATA
CORR 1 .5
CORR .5 1
END DATA.
");
    assert_eq!(cases_of(&session), ["CORR a 1 0.5", "CORR b 0.5 1"]);
    assert!(
        warnings[0].contains("line 3: 1 values after the end"),
        "{warnings:?}"
    );
    let e = fails("MATRIX DATA VARIABLES=ROWTYPE_ a b.\nBEGIN DATA\nCORR 1\nCORR x 1\nEND DATA.\n");
    assert!(e.contains("line 4: x is not a number"), "{e}");
    let e = fails("MATRIX DATA VARIABLES=ROWTYPE_ a b.\nBEGIN DATA\nCOR 1\nMEN 1 2\nEND DATA.\n");
    assert!(e.contains("line 4: MEN is no record type"), "{e}");
    let e =
        fails("MATRIX DATA VARIABLES=ROWTYPE_ a b c.\nBEGIN DATA\nCORR 1\nCORR .5 1\nEND DATA.\n");
    assert!(e.contains("the CORR matrix has 2 of its 3 records"), "{e}");
    let e = fails("MATRIX DATA VARIABLES=a b /CONTENTS=MEAN.\nBEGIN DATA\n1 2\n3 4\nEND DATA.\n");
    assert!(e.contains("give SPLIT"), "{e}");
    let e = fails("MATRIX DATA VARIABLES=a b /CONTENTS=MEAN N.\nBEGIN DATA\n1 2\n3\nEND DATA.\n");
    assert!(e.contains("the data end where N of b should follow"), "{e}");
    // without ROWTYPE_ and CONTENTS every record is a correlation row, with a warning
    let (session, _, warnings) =
        run("MATRIX DATA VARIABLES=a b.\nBEGIN DATA\n1\n.5 1\nEND DATA.\n");
    assert_eq!(cases_of(&session).len(), 2);
    assert!(
        warnings[0].contains("every record is read as a row"),
        "{warnings:?}"
    );
}

#[test]
fn data_from_a_file() {
    let dir = std::env::temp_dir().join(format!("statscraft-matrix-data-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("rawmtx.txt"), "MEAN 1 2\nCORR 1\nCORR .25 1\n").unwrap();
    let (session, _, _) = run(&format!(
        "CD '{}'.\nMATRIX DATA FILE='rawmtx.txt' /VARIABLES=ROWTYPE_ a b.\nLIST.\n",
        dir.display()
    ));
    assert_eq!(
        cases_of(&session),
        ["MEAN  1 2", "CORR a 1 0.25", "CORR b 0.25 1"]
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn factor_reads_the_matrix() {
    let (_, tables, _) = run(
        "MATRIX DATA VARIABLES=ROWTYPE_ x1 TO x3 /FORMAT=LOWER NODIAGONAL.
BEGIN DATA
N 100 100 100
CORR .5
CORR .5 .5
END DATA.
FACTOR MATRIX=IN(COR=*) /PRINT=INITIAL EXTRACTION /CRITERIA=FACTORS(1).
",
    );
    let tve = find(&tables, "Total Variance Explained");
    let eigen: Vec<f64> = tve.rows.iter().map(|r| number(&r[1])).collect();
    for (ours, expected) in eigen.iter().zip([2.0, 0.5, 0.5]) {
        assert!((ours - expected).abs() < 1e-12, "{eigen:?}");
    }
    // the share of the first: 2 of 3
    assert!((number(&tve.rows[0][2]) - 200.0 / 3.0).abs() < 1e-9);
}

#[test]
fn cluster_reads_a_proximity_matrix() {
    // distances a-b 1, a-c 4, b-c 3: single linkage joins a and b at 1, then c at 3
    let (_, tables, _) = run(
        "MATRIX DATA VARIABLES=a b c /CONTENTS=PROX /FORMAT=NODIAGONAL.
BEGIN DATA
1
4 3
END DATA.
VALUE LABELS ROWTYPE_ 'PROX' 'DISSIMILARITY'.
CLUSTER /MATRIX=IN(*) /METHOD=SINGLE /PRINT=SCHEDULE.
",
    );
    let schedule = find(&tables, "Agglomeration Schedule");
    let stages: Vec<(f64, f64, f64)> = schedule
        .rows
        .iter()
        .map(|r| (number(&r[1]), number(&r[2]), number(&r[3])))
        .collect();
    assert_eq!(stages, [(1.0, 2.0, 1.0), (1.0, 3.0, 3.0)]);
}
