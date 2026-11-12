//! MATRIX DATA on synthetic matrix materials in each layout: the matrix file it builds, value
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

/// The materials most tests read: means 12 7 30, standard deviations 2 1.5 4, 40 cases and the
/// correlations .25 (Q1, Q2), .4 (Q1, Q3) and -.3 (Q2, Q3).
const BASE: [&str; 6] = [
    "MEAN  12 7 30",
    "STDDEV  2 1.5 4",
    "N  40 40 40",
    "CORR Q1 1 0.25 0.4",
    "CORR Q2 0.25 1 -0.3",
    "CORR Q3 0.4 -0.3 1",
];

#[test]
fn record_types_in_the_data() {
    let (session, _, _) = run("MATRIX DATA VARIABLES=ROWTYPE_ Q1 TO Q3.
BEGIN DATA
'MEAN    ' 12 7 30
SD 2 1.5 4
\"N\" 40 40 40
CORR 1
CORR .25 1
CORR .4 -.3 1
END DATA.
");
    let data = session.dataset().unwrap();
    assert_eq!(
        names(data),
        [
            "ROWTYPE_ A8",
            "VARNAME_ A8",
            "Q1 F10.4",
            "Q2 F10.4",
            "Q3 F10.4"
        ]
    );
    assert_eq!(cases(data), BASE);
}

#[test]
fn shapes_and_diagonals_give_the_same_matrix() {
    let full = "CORR 1 .25 .4\nCORR .25 1 -.3\nCORR .4 -.3 1";
    for (format, rows) in [
        ("FULL", full),
        ("UPPER", "CORR 1 .25 .4\nCORR 1 -.3\nCORR 1"),
        ("UPPER NODIAGONAL", "CORR .25 .4\nCORR -.3"),
        ("LOWER NODIAGONAL", "CORR .25\nCORR .4 -.3"),
        ("LOWER DIAGONAL", "CORR 1\nCORR .25 1\nCORR .4 -.3 1"),
    ] {
        let (session, _, _) = run(&format!(
            "MATRIX DATA VARIABLES=ROWTYPE_ Q1 TO Q3 /FORMAT={format}.\nBEGIN DATA\nN 40 40 40\n{rows}\nEND DATA.\n"
        ));
        assert_eq!(
            cases(session.dataset().unwrap()),
            BASE[2..],
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
        "MATRIX DATA VARIABLES=Q1 TO Q3 /CONTENTS=MEAN SD N_SCALAR CORR.
BEGIN DATA
12 7
 30
2,1.5,4
40
1
.25+1
.4-.3 1
END DATA.
",
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(cases(session.dataset().unwrap()), BASE);
    let (session, _, _) = run(
        "MATRIX DATA VARIABLES=Q1 TO Q3 /CONTENTS=MEAN SD CORR /N=55.
BEGIN DATA
12 7 30
4 3 2.5
1
.25 1
.4 -.3 1
END DATA.
",
    );
    let data = session.dataset().unwrap();
    assert_eq!(cases(data)[0], "N  55 55 55");
    assert_eq!(cases(data)[2], "STDDEV  4 3 2.5");
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
        "MEAN 1  10 20 30",
        "STDDEV 1  1 2 3",
        "N 1  15 15 15",
        "MEAN 2  11 18 33",
        "STDDEV 2  1.2 2.5 2.8",
        "N 2  12 12 12",
        "CORR . X1 1 0.25 0.4",
        "CORR . X2 0.25 1 -0.3",
        "CORR . X3 0.4 -0.3 1",
    ];
    let (session, _, _) = run("MATRIX DATA VARIABLES=GRP X1 TO X3 /FACTORS=GRP /CELLS=2
  /CONTENTS=(MEAN SD N) CORR.
BEGIN DATA
1 10 20 30
1 1 2 3
1 15 15 15
2 11 18 33
2 1.2 2.5 2.8
2 12 12 12
   1
  .25 1
  .4 -.3 1
END DATA.
");
    let data = session.dataset().unwrap();
    assert_eq!(
        names(data),
        [
            "ROWTYPE_ A8",
            "GRP F4.0",
            "VARNAME_ A8",
            "X1 F10.4",
            "X2 F10.4",
            "X3 F10.4"
        ]
    );
    assert_eq!(cases(data), expected);
    // the same records with ROWTYPE_ in the data, CELLS ignored with a warning
    let (session, _, warnings) = run(
        "MATRIX DATA VARIABLES=ROWTYPE_ GRP X1 TO X3 /FACTORS=GRP /CELLS=2.
BEGIN DATA
MEAN 1 10 20 30
SD   1 1 2 3
N    1 15 15 15
MEAN 2 11 18 33
SD   2 1.2 2.5 2.8
N    2 12 12 12
CORR . 1
CORR . .25 1
CORR . .4 -.3 1
END DATA.
",
    );
    assert_eq!(cases(session.dataset().unwrap()), expected);
    assert!(warnings[0].contains("CELLS is ignored"), "{warnings:?}");
    // each type grouped across the cells
    let (session, _, _) = run("MATRIX DATA VARIABLES=GRP X1 TO X3 /FACTORS=GRP /CELLS=2
  /CONTENTS=(MEAN) (SD) (N) CORR.
BEGIN DATA
1 10 20 30
2 11 18 33
1 1 2 3
2 1.2 2.5 2.8
1 15 15 15
2 12 12 12
   1
  .25 1
  .4 -.3 1
END DATA.
");
    let data = session.dataset().unwrap();
    assert_eq!(cases(data)[1], "MEAN 2  11 18 33");
    assert_eq!(cases(data)[2], "STDDEV 1  1 2 3");
}

#[test]
fn split_files() {
    let (session, _, _) = run(
        "MATRIX DATA VARIABLES=Q1 TO Q3 /CONTENTS=MEAN SD N CORR /SPLIT=WAVE.
BEGIN DATA
  12 7 30
  2 1.5 4
  40 40 40
  1
 .25 1
 .4 -.3 1
  14 6 28
  2.5 1 3
  35 35 35
  1
 .1 1
 .2 .15 1
END DATA.
",
    );
    let data = session.dataset().unwrap();
    assert_eq!(names(data)[0], "WAVE F4.0");
    let cases = cases(data);
    assert_eq!(cases.len(), 12);
    assert_eq!(cases[0], "1 MEAN  12 7 30");
    assert_eq!(cases[11], "2 CORR Q3 0.2 0.15 1");
    assert_eq!(
        data.split_file().map(|s| s.variables.clone()),
        Some(vec!["WAVE".to_string()])
    );
    let (session, _, _) = run("MATRIX DATA VARIABLES=SITE ROWTYPE_ U1 TO U2 /SPLIT=SITE.
BEGIN DATA
3 MEAN 2 6
3 CORR 1
3 CORR -.5 1
4 MEAN 7 1
4 CORR 1
4 CORR .45 1
END DATA.
");
    assert_eq!(
        cases_of(&session)[3..],
        ["4 MEAN  7 1", "4 CORR U1 1 0.45", "4 CORR U2 0.45 1"]
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
