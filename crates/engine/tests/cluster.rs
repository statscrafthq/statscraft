//! CLUSTER on synthetic data. The expected numbers come from an independent computation from
//! the raw values: every step joins the pair of clusters that is closest by the definition of
//! the method, measured over the original items (the mean proximity between or within the
//! clusters, the nearest or farthest pair, the squared distance between centroids, the rise of
//! the within-cluster sum of squares for Ward), in exact rational arithmetic, cross-checked
//! with a general numerical library's hierarchical clustering (which also gives MEDIAN).

use statscraft_data::Values;
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

/// Twelve cases; G misses z, so eleven are clustered. b1 to b5 are presence and absence.
const DATA: &str = "DATA LIST FREE /name (A1) x y z b1 b2 b3 b4 b5.
BEGIN DATA.
A 1.2 2.1 3.0 1 0 1 1 0
B 2.3 1.9 4.1 1 1 1 0 0
C 8.1 9.4 7.2 0 0 1 1 1
D 9.0 8.2 8.3 0 1 1 1 1
E 0.8 1.1 2.4 1 0 0 1 0
F 5.1 4.7 5.3 1 1 0 0 1
G 4.2 6.1 .   0 1 0 1 1
H 7.7 8.5 9.1 0 0 0 1 1
I 2.0 3.2 2.9 1 0 1 0 0
J 5.5 4.1 6.2 1 1 1 1 0
K 6.9 9.0 7.6 0 1 0 0 1
L 3.9 5.2 4.8 1 0 0 1 1
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

fn error(syntax: &str) -> String {
    let mut engine = Engine::default();
    let (commands, errors) = parse(&format!("{DATA}{syntax}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut last = None;
    for command in &commands {
        if let Err(e) = engine.execute(&command.command) {
            last = Some(e.to_string());
        }
    }
    last.expect("an error")
}

fn number(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        other => panic!("not a number: {other:?}"),
    }
}

/// (cluster 1, cluster 2, coefficient) of every stage.
fn schedule(table: &Table) -> Vec<(usize, usize, f64)> {
    table
        .rows
        .iter()
        .map(|r| {
            (
                number(&r[1]) as usize,
                number(&r[2]) as usize,
                number(&r[3]),
            )
        })
        .collect()
}

fn assert_schedule(table: &Table, expected: &[(usize, usize, f64)]) {
    let ours = schedule(table);
    assert_eq!(
        ours.len(),
        expected.len(),
        "{}",
        table.caption.as_deref().unwrap_or("")
    );
    for (k, (a, e)) in ours.iter().zip(expected).enumerate() {
        assert_eq!((a.0, a.1), (e.0, e.1), "stage {}: {ours:?}", k + 1);
        assert!(
            (a.2 - e.2).abs() <= 1e-9 * e.2.abs().max(1.0),
            "stage {} coefficient: ours {}, expected {}",
            k + 1,
            a.2,
            e.2
        );
    }
}

fn schedules(tables: &[Table]) -> Vec<&Table> {
    tables
        .iter()
        .filter(|t| t.title == "Agglomeration Schedule")
        .collect()
}

#[test]
fn the_seven_methods_on_squared_euclidean_distances() {
    let tables =
        run("CLUSTER x y z /METHOD=BAVERAGE WAVERAGE SINGLE COMPLETE CENTROID MEDIAN WARD.");
    let s = schedules(&tables);
    assert_eq!(s.len(), 7);
    assert_schedule(
        s[0],
        &[
            (6, 9, 1.33),
            (1, 5, 1.52),
            (3, 10, 1.76),
            (4, 7, 2.42),
            (2, 8, 3.22),
            (6, 11, 3.835),
            (1, 2, 4.05),
            (3, 4, 4.18),
            (1, 6, 25.086666666666666),
            (1, 3, 79.44142857142857),
        ],
    );
    assert_schedule(
        s[1],
        &[
            (6, 9, 1.33),
            (1, 5, 1.52),
            (3, 10, 1.76),
            (4, 7, 2.42),
            (6, 11, 3.0),
            (1, 8, 3.16),
            (3, 4, 3.4833333333333334),
            (1, 2, 3.49),
            (1, 6, 15.76095238095238),
            (1, 3, 46.84072727272727),
        ],
    );
    assert_schedule(
        s[2],
        &[
            (6, 9, 1.33),
            (1, 5, 1.52),
            (3, 10, 1.76),
            (1, 8, 1.86),
            (6, 11, 1.94),
            (4, 7, 2.42),
            (1, 2, 2.46),
            (3, 4, 314.0 / 100.0),
            (1, 6, 11.22),
            (1, 3, 27.02),
        ],
    );
    assert_schedule(
        s[3],
        &[
            (6, 9, 1.33),
            (1, 5, 1.52),
            (3, 10, 1.76),
            (4, 7, 2.42),
            (2, 8, 3.22),
            (3, 4, 5.54),
            (6, 11, 5.73),
            (1, 2, 6.1),
            (1, 6, 45.53),
            (1, 3, 152.46),
        ],
    );
    assert_schedule(
        s[4],
        &[
            (6, 9, 1.33),
            (1, 5, 1.52),
            (3, 10, 1.76),
            (4, 7, 2.42),
            (3, 4, 3.135),
            (2, 8, 3.22),
            (1, 2, 2.865),
            (6, 11, 3.5025),
            (1, 6, 22.777916666666666),
            (1, 3, 71.38048469387755),
        ],
    );
    // the median method: the heights of the library, squared, in order
    let mut median: Vec<f64> = schedule(s[5]).iter().map(|r| r.2).collect();
    median.sort_by(f64::total_cmp);
    let expected = [
        1.33,
        1.52,
        1.76,
        2.42,
        2.865,
        3.135,
        3.22,
        3.5025,
        21.306875,
        66.80921875,
    ];
    for (a, e) in median.iter().zip(expected) {
        assert!((a - e).abs() < 1e-9, "median: {median:?}");
    }
    assert_schedule(
        s[6],
        &[
            (6, 9, 0.665),
            (1, 5, 1.425),
            (3, 10, 2.305),
            (4, 7, 3.515),
            (2, 8, 5.125),
            (6, 11, 7.46),
            (1, 2, 10.325),
            (3, 4, 13.46),
            (1, 6, 52.50785714285714),
            (1, 3, 234.20363636363638),
        ],
    );
    // stage cluster first appears and next stage, for the first method
    let rows = &s[0].rows;
    let at = |r: usize| -> Vec<f64> { rows[r][4..].iter().map(number).collect() };
    assert_eq!(at(0), [0.0, 0.0, 6.0]);
    assert_eq!(at(5), [1.0, 0.0, 9.0]);
    assert_eq!(at(9), [9.0, 8.0, 0.0]);
    assert_eq!(
        s[0].caption.as_deref(),
        Some("Average Linkage (Between Groups)")
    );
    // eleven of twelve cases
    let summary = tables
        .iter()
        .find(|t| t.title == "Case Processing Summary")
        .unwrap();
    assert_eq!(number(&summary.rows[0][0]), 11.0);
    assert_eq!(number(&summary.rows[0][2]), 1.0);
    assert_eq!(
        summary.caption.as_deref(),
        Some("Squared Euclidean Distance used")
    );
}

#[test]
fn membership_and_saved_clusters() {
    let mut engine = Engine::default();
    let tables = run_on(
        &mut engine,
        &format!(
            "{DATA}CLUSTER x y z /METHOD=WARD BAVERAGE(avg) /ID=name /PRINT=CLUSTER(2,4) NONE /PRINT=CLUSTER(2,4) \
             /SAVE=CLUSTER(2,3)."
        ),
    );
    // PRINT=CLUSTER alone leaves out the schedule
    assert!(schedules(&tables).is_empty());
    let member = tables
        .iter()
        .find(|t| t.title == "Cluster Membership")
        .unwrap();
    assert_eq!(
        member.columns[1..],
        ["4 Clusters", "3 Clusters", "2 Clusters"]
    );
    let column =
        |k: usize| -> Vec<usize> { member.rows.iter().map(|r| number(&r[k]) as usize).collect() };
    assert_eq!(column(1), [1, 1, 2, 3, 1, 4, 3, 1, 4, 2, 4]);
    assert_eq!(column(2), [1, 1, 2, 2, 1, 3, 2, 1, 3, 2, 3]);
    assert_eq!(column(3), [1, 1, 2, 2, 1, 1, 2, 1, 1, 2, 1]);
    assert_eq!(member.rows[6][0], Cell::Text("8:H".into()));
    let data = engine.dataset().unwrap();
    let values = |name: &str| match data.variable(name).unwrap().values() {
        Values::Numeric(v) => (0..12).map(|i| v.get(i)).collect::<Vec<_>>(),
        Values::String(_) => panic!(),
    };
    // Ward has the default names; G, missing on z, gets no cluster
    let ward3 = values("CLU3_1");
    assert_eq!(ward3[0], Some(1.0));
    assert_eq!(ward3[6], None);
    assert_eq!(ward3[11], Some(3.0));
    assert_eq!(values("CLU2_1")[2], Some(2.0));
    // the rootname of BAVERAGE
    let avg3: Vec<Option<f64>> = values("avg3");
    let expected = [1, 1, 2, 2, 1, 3, 0, 2, 1, 3, 2, 3];
    for (v, e) in avg3.iter().zip(expected) {
        assert_eq!(*v, (e > 0).then_some(e as f64));
    }
    let order: Vec<&str> = data.variables().map(|v| v.name()).collect::<Vec<_>>()[9..].to_vec();
    assert_eq!(order, ["CLU3_1", "CLU2_1", "avg3", "avg2"]);
    // a second run takes the next free names
    run_on(&mut engine, "CLUSTER x y z /SAVE=CLUSTER(2).");
    assert!(engine.dataset().unwrap().variable("CLU2_2").is_ok());
}

#[test]
fn similarity_measures_join_the_most_similar_pair() {
    let tables = run("CLUSTER x y z /MEASURE=CORRELATION.");
    assert_schedule(
        schedules(&tables)[0],
        &[
            (6, 9, 0.9999999999999998),
            (8, 11, 0.998046095756055),
            (1, 7, 0.996615895540124),
            (2, 5, 0.939783563231742),
            (8, 10, 0.904352523347939),
            (1, 2, 0.8327290892932009),
            (1, 6, 0.5152645149951547),
            (3, 8, 0.48950987678094265),
            (1, 3, -0.14275455206698004),
            (1, 4, -0.4857006205085548),
        ],
    );
    let tables = run("CLUSTER b1 TO b5 /MISSING=EXCLUDE /MEASURE=JACCARD /METHOD=COMPLETE SINGLE.");
    // the binary variables have no missing value: all twelve cases, G is item 7
    let s = schedules(&tables);
    let ours = schedule(s[0]);
    assert_eq!(ours.len(), 11);
    let tables = run("COMPUTE keep = NOT(MISSING(z)).\nFILTER BY keep.\n\
         CLUSTER b1 TO b5 /MEASURE=JACCARD(1,0) /METHOD=COMPLETE SINGLE.");
    let s = schedules(&tables);
    let third = 2.0 / 3.0;
    // for a similarity, complete linkage keeps the smaller similarity, single the larger
    assert_schedule(
        s[0],
        &[
            (1, 9, 0.75),
            (3, 4, 0.75),
            (2, 8, third),
            (5, 11, third),
            (6, 10, third),
            (1, 2, 0.5),
            (3, 7, 0.5),
            (1, 5, 0.2),
            (3, 6, 0.2),
            (1, 3, 0.0),
        ],
    );
    assert_schedule(
        s[1],
        &[
            (1, 9, 0.75),
            (1, 2, 0.75),
            (3, 4, 0.75),
            (1, 5, third),
            (1, 8, third),
            (1, 11, third),
            (1, 7, third),
            (1, 3, third),
            (6, 10, third),
            (1, 6, 0.5),
        ],
    );
}

#[test]
fn interval_and_count_measures_in_the_proximity_matrix() {
    // A and B: the first two cases
    for (measure, expected) in [
        ("SEUCLID", 2.46),
        ("EUCLID", 1.5684387141358118),
        ("BLOCK", 2.3999999999999995),
        ("CHEBYCHEV", 1.0999999999999999),
        ("MINKOWSKI(3)", 1.387300109179834),
        ("POWER(3,2)", 1.6340134638368187),
        ("COSINE", 0.9749463436128714),
        ("CHISQ", 0.5069384931348335),
        ("PH2", 0.13267186729430336),
    ] {
        let tables = run(&format!(
            "CLUSTER x y z /MEASURE={measure} /PRINT=DISTANCE /ID=name."
        ));
        let m = tables
            .iter()
            .find(|t| t.title == "Proximity Matrix")
            .unwrap();
        assert_eq!(m.columns[1], "1:A");
        assert_eq!(m.rows[0][1], Cell::Empty);
        let v = number(&m.rows[0][2]);
        assert!((v - expected).abs() < 1e-12, "{measure}: {v}");
        assert_eq!(number(&m.rows[1][1]), v);
    }
}

#[test]
fn binary_measures_from_the_two_by_two_table() {
    // A = 1 0 1 1 0 and B = 1 1 1 0 0: a = 2, b = 1, c = 1, d = 1
    let (a, b, c, d) = (2.0_f64, 1.0_f64, 1.0_f64, 1.0_f64);
    let n = a + b + c + d;
    let t1 = a.max(b) + c.max(d) + a.max(c) + b.max(d);
    let t2 = (a + c).max(b + d) + (a + b).max(c + d);
    let m = ((a + b) * (a + c) * (b + d) * (c + d)).sqrt();
    for (measure, expected) in [
        ("RR", a / n),
        ("SM", (a + d) / n),
        ("JACCARD", a / (a + b + c)),
        ("DICE", 2.0 * a / (2.0 * a + b + c)),
        ("SS1", 2.0 * (a + d) / (2.0 * (a + d) + b + c)),
        ("RT", (a + d) / (a + d + 2.0 * (b + c))),
        ("SS2", a / (a + 2.0 * (b + c))),
        ("K1", a / (b + c)),
        ("SS3", (a + d) / (b + c)),
        ("K2", (a / (a + b) + a / (a + c)) / 2.0),
        (
            "SS4",
            (a / (a + b) + a / (a + c) + d / (b + d) + d / (c + d)) / 4.0,
        ),
        ("HAMANN", ((a + d) - (b + c)) / n),
        ("LAMBDA", (t1 - t2) / (2.0 * n - t2)),
        ("D", (t1 - t2) / (2.0 * n)),
        (
            "Y",
            ((a * d).sqrt() - (b * c).sqrt()) / ((a * d).sqrt() + (b * c).sqrt()),
        ),
        ("Q", (a * d - b * c) / (a * d + b * c)),
        ("OCHIAI", (a / (a + b) * a / (a + c)).sqrt()),
        ("SS5", a * d / m),
        ("PHI", (a * d - b * c) / m),
        ("BEUCLID", (b + c).sqrt()),
        ("BSEUCLID", b + c),
        ("SIZE", (b - c).powi(2) / (n * n)),
        ("PATTERN", b * c / (n * n)),
        ("BSHAPE", (n * (b + c) - (b - c).powi(2)) / (n * n)),
        ("DISPER", (a * d - b * c) / (n * n)),
        ("VARIANCE", (b + c) / (4.0 * n)),
        ("BLWMN", (b + c) / (2.0 * a + b + c)),
    ] {
        // b1 to b5 are 0 and 1: as presence 1 and absence 0, and also as presence 0 reversed
        let tables = run(&format!(
            "COMPUTE keep = $CASENUM LE 2.\nFILTER BY keep.\n\
             CLUSTER b1 TO b5 /MEASURE={measure} /PRINT=DISTANCE."
        ));
        let matrix = tables
            .iter()
            .find(|t| t.title == "Proximity Matrix")
            .unwrap();
        let v = number(&matrix.rows[0][2]);
        assert!(
            (v - expected).abs() < 1e-12,
            "{measure}: {v}, expected {expected}"
        );
    }
    // presence 0 alone: every other value is absence, so the table turns over
    let tables = run("COMPUTE keep = $CASENUM LE 2.\nFILTER BY keep.\n\
         CLUSTER b1 TO b5 /MEASURE=JACCARD(0) /PRINT=DISTANCE.");
    let matrix = tables
        .iter()
        .find(|t| t.title == "Proximity Matrix")
        .unwrap();
    // presence 0: a = 1 (b5), b = 1 (b4), c = 1 (b2)
    assert!((number(&matrix.rows[0][2]) - 1.0 / 3.0).abs() < 1e-12);
    // with both values given, a value that is neither is skipped
    let tables = run(
        "COMPUTE keep = $CASENUM LE 2.\nFILTER BY keep.\nRECODE b2 (1=9).\n\
         CLUSTER b1 TO b5 /MEASURE=SM(1,0) /PRINT=DISTANCE.",
    );
    let matrix = tables
        .iter()
        .find(|t| t.title == "Proximity Matrix")
        .unwrap();
    // b2 drops out: a = 2, b = 1, c = 0, d = 1
    assert!((number(&matrix.rows[0][2]) - 0.75).abs() < 1e-12);
}

#[test]
fn matrix_input_clusters_the_items_of_the_file() {
    // the squared Euclidean distances of A, B, C and E as a lower triangle
    let source = "DATA LIST FREE /ROWTYPE_ (A8) VARNAME_ (A8) VAR1 VAR2 VAR3 VAR4.
BEGIN DATA.
PROX VAR1 0 . . .
PROX VAR2 2.46 0 . .
PROX VAR3 115.82 108.83 0 .
PROX VAR4 2.0 5.69 143.45 0
END DATA.
CLUSTER /MATRIX=IN(*) /METHOD=SINGLE /PRINT=SCHEDULE CLUSTER(2).
";
    let tables = run_on(&mut Engine::default(), source);
    assert_schedule(
        schedules(&tables)[0],
        &[(1, 4, 2.0), (1, 2, 2.46), (1, 3, 108.83)],
    );
    let member = tables
        .iter()
        .find(|t| t.title == "Cluster Membership")
        .unwrap();
    assert_eq!(member.rows[2][0], Cell::Text("VAR3".into()));
    assert_eq!(number(&member.rows[2][1]), 2.0);
    // a value label of PROX makes it a similarity matrix; a list picks items
    let source = source.replace(
        "CLUSTER /MATRIX",
        "VALUE LABELS ROWTYPE_ 'PROX' 'SIMILARITY'.\nCLUSTER VAR1 VAR2 VAR3 /MATRIX",
    );
    let tables = run_on(&mut Engine::default(), &source);
    assert_schedule(schedules(&tables)[0], &[(1, 3, 115.82), (1, 2, 108.83)]);
}

#[test]
fn errors_and_warnings() {
    let mut engine = Engine::default();
    run_on(
        &mut engine,
        &format!("{DATA}WEIGHT BY x.\nCLUSTER x y z /PLOT=DENDROGRAM."),
    );
    let warnings = engine.warnings().join(" | ");
    assert!(warnings.contains("weight"), "{warnings}");
    assert_eq!(engine.take_charts().len(), 1);
    assert!(error("CLUSTER name x.").contains("string variable"));
    // all zero vectors: the Jaccard measure of two cases without presence is undefined
    let e = error(
        "COMPUTE keep = $CASENUM LE 2.\nFILTER BY keep.\nRECODE b1 TO b5 (ELSE=0).\n\
         CLUSTER b1 TO b5 /MEASURE=JACCARD.",
    );
    assert!(e.contains("undefined"), "{e}");
    let e = error("CLUSTER x y /SAVE=CLUSTER(2) /METHOD=SINGLE(b).");
    assert!(e.contains("already exists"), "{e}");
}
