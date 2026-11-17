//! PREFSCAL on invented preference data. The expected numbers come from an independent base R
//! implementation of the same penalized stress iteration (written separately from this code,
//! from the same written rules), started from the same classical scaling start; distances
//! are compared because rotations and reflections leave them unchanged.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

/// Eight made-up tasters' distances to five snacks, from points in a plane with noise.
const TASTE: &str = "DATA LIST FREE /c1 TO c5.
BEGIN DATA.
3.4 4.6 2.3 1.8 5.5
3.3 7.1 7.3 3.6 2.2
1.6 3.5 5.7 4.3 2.4
2.8 5.7 5.2 2.3 3.3
3.6 5.7 3.5 1.4 6.9
4.7 2.1 2.5 6.4 7.5
2.7 5.3 3.5 0.2 4.4
1.4 4.1 3.2 1.3 4.4
END DATA.
";

/// Two judges rate four items for the same six people, each judge stretching the two axes of
/// one layout differently, with noise.
const JUDGES: &str = "DATA LIST FREE /i1 TO i4.
BEGIN DATA.
1.4 4.2 3.3 1.7
2.2 1 0.7 5.1
4.1 1 2.6 7.3
1.2 4 3.5 2.8
2.6 5.2 4.5 0.9
2.1 0.2 1.6 5
1.1 1.6 3.5 0.6
3.2 0.9 1.9 2.5
4.8 2.1 0.9 4.1
2.3 3.7 6.4 3.5
1.6 3.5 5 2.5
1.4 0.6 3.4 2.1
END DATA.
";

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

fn close(got: f64, want: f64, what: &str) {
    assert!((got - want).abs() < 1e-6, "{what}: {got} against {want}");
}

/// Checks the Measures table (after the iterations row) and the distances of the first two
/// rows in every source's distance table.
fn check(tables: &[Table], measures: &[f64], distances: &[(&str, &[f64])]) {
    let rows = &table(tables, "Measures").rows;
    for (row, want) in rows[1..].iter().zip(measures) {
        close(number(&row[1]), *want, &format!("{row:?}"));
    }
    for (title, want) in distances {
        let t = table(tables, title);
        let got: Vec<f64> = t.rows[..2]
            .iter()
            .flat_map(|r| r[1..].iter().map(number))
            .collect();
        assert_eq!(got.len(), want.len(), "{title}");
        for (g, w) in got.iter().zip(*want) {
            close(*g, *w, title);
        }
    }
}

const ALL: &str = "/PRINT=MEASURES COMMON DISTANCES";

#[test]
fn ordinal_unconditional_with_the_defaults_matches_the_oracle() {
    let tables = run(
        TASTE,
        &format!("PREFSCAL c1 TO c5 /CONDITION=UNCONDITIONAL {ALL}."),
    );
    assert_eq!(number(&table(&tables, "Measures").rows[0][1]), 427.0);
    check(
        &tables,
        &[
            0.0247205662,
            0.0121584233,
            2.0332049375,
            0.0001478273,
            0.0121593220,
            0.0288389385,
            0.0190276853,
            0.0299537774,
            0.9998521727,
            0.9991693069,
            0.9750000000,
            0.9750000000,
            0.9500000000,
            0.4729254650,
            0.4652640139,
            0.4649792955,
        ],
        &[(
            "Distances",
            &[
                3.48513335, 4.08285498, 1.94282416, 1.89008129, 5.86287105, 3.42325034, 6.95065560,
                7.28285260, 4.04355008, 1.86165994,
            ],
        )],
    );
}

#[test]
fn linear_and_spline_transformations_match_the_oracle() {
    let cases: [(&str, [f64; 16], [f64; 10]); 3] = [
        (
            "/TRANSFORMATION=LINEAR(INTERCEPT) /CRITERIA=MAXITER(200)",
            [
                0.0369694428,
                0.0175059153,
                2.1118257617,
                0.0003064571,
                0.0185881168,
                0.0449845934,
                0.0373959719,
                0.0589384375,
                0.9996935429,
                0.9979820778,
                1.0,
                1.0,
                1.0,
                0.4729254650,
                0.4544388821,
                0.4537608773,
            ],
            [
                4.02713173, 5.50683288, 2.72997103, 2.11833683, 6.62474484, 3.11122397, 6.28446759,
                6.59548499, 3.31722114, 2.15529702,
            ],
        ),
        (
            "/CONDITION=MATRIX /TRANSFORMATION=SPLINE(INTERCEPT ORDER=2 INKNOT=1)",
            [
                0.0878408855,
                0.0426161999,
                2.0612087816,
                0.0018161405,
                0.0426549509,
                0.1029020634,
                0.0756195120,
                0.1198150090,
                0.9981838595,
                0.9895722315,
                1.0,
                1.0,
                1.0,
                0.4729254650,
                0.4590841321,
                0.4554962005,
            ],
            [
                3.15009788, 4.75847218, 2.34511903, 1.82024768, 5.89428015, 3.31016235, 6.73492021,
                6.99839950, 3.71774347, 2.22408151,
            ],
        ),
        (
            "/TRANSFORMATION=SPLINE /CRITERIA=MAXITER(300)",
            [
                0.0002864624,
                0.0001383205,
                2.0710042219,
                0.0000000191,
                0.0001418714,
                0.0003456301,
                0.0002208512,
                0.0003561413,
                0.9999999809,
                0.9999998807,
                1.0,
                1.0,
                1.0,
                0.4729254650,
                0.4501358901,
                0.4501413208,
            ],
            [
                3.47867871, 4.78084751, 2.57678216, 2.09711140, 6.31857960, 3.15744415, 6.37942187,
                6.68029745, 3.26511388, 2.50473918,
            ],
        ),
    ];
    for (options, measures, distances) in cases {
        let tables = run(TASTE, &format!("PREFSCAL c1 TO c5 {options} {ALL}."));
        check(&tables, &measures, &[("Distances", &distances)]);
    }
}

#[test]
fn similarities_untied_and_a_stronger_penalty_match_the_oracle() {
    let tables = run(
        TASTE,
        &format!(
            "PREFSCAL c1 TO c5 /PROXIMITIES=SIMILARITIES /CONDITION=MATRIX \
             /TRANSFORMATION=ORDINAL(UNTIE) /PENALTY=LAMBDA(0.7) OMEGA(2) {ALL}."
        ),
    );
    check(
        &tables,
        &[
            0.0549395817,
            0.0084746191,
            6.4828379020,
            0.0010969118,
            0.0331378407,
            0.1239483267,
            0.0604617692,
            0.1352078053,
            0.9989030882,
            0.9850157508,
            0.9,
            0.9,
            0.8,
            0.4650078462,
            0.2808488019,
            0.2774516001,
        ],
        &[(
            "Distances",
            &[
                4.63922353, 3.22262201, 5.03448809, 5.32001387, 2.75905197, 5.47320270, 2.89937884,
                2.28098668, 4.71749398, 4.89830526,
            ],
        )],
    );
}

#[test]
fn untransformed_proximities_match_the_oracle() {
    // NONE keeps the proximities, so the penalty is 1 + OMEGA exactly
    let tables = run(
        TASTE,
        &format!("PREFSCAL c1 TO c5 /TRANSFORMATION=NONE {ALL} TRANSFORMATION."),
    );
    check(
        &tables,
        &[
            0.1007175589,
            0.0503587794,
            2.0,
            0.0025360067,
            0.0507595323,
            0.1205303724,
            0.0921588536,
            0.1442627442,
            0.9974639933,
            0.9861081125,
            1.0,
            1.0,
            1.0,
            0.4729254650,
            0.4729254650,
            0.4643171783,
        ],
        &[(
            "Distances",
            &[
                3.11318254, 4.60991509, 2.28529848, 1.65663607, 5.86077886, 3.58541973, 6.88287447,
                7.08973922, 3.74177755, 2.24752522,
            ],
        )],
    );
    let t = table(&tables, "Transformed Proximities");
    assert_eq!(number(&t.rows[0][1]), 3.4);
    let tables = run(
        TASTE,
        &format!(
            "PREFSCAL c1 TO c5 /TRANSFORMATION=NONE(INTERCEPT) /CONDITION=UNCONDITIONAL \
             /CRITERIA=MAXITER(400) {ALL}."
        ),
    );
    check(
        &tables,
        &[
            0.0995608795,
            0.0489852034,
            2.0324684311,
            0.0023995502,
            0.0490455613,
            0.1172097613,
            0.0863140988,
            0.1355100067,
            0.9976004498,
            0.9865328511,
            1.0,
            1.0,
            1.0,
            0.4729254650,
            0.4654299313,
            0.4607164923,
        ],
        &[(
            "Distances",
            &[
                3.14776698, 4.65793037, 2.37555412, 1.73722141, 5.94624250, 3.61392541, 6.95238004,
                7.19222814, 3.78364987, 2.31637692,
            ],
        )],
    );
}

#[test]
fn the_weighted_model_matches_the_oracle() {
    type Case<'a> = (&'a str, [f64; 16], [f64; 8], [f64; 8]);
    let cases: [Case; 2] = [
        (
            "/TRANSFORMATION=LINEAR /CONDITION=UNCONDITIONAL",
            [
                0.1247901584,
                0.0623950792,
                2.0,
                0.0038931459,
                0.0625168916,
                0.1242981862,
                0.0891554065,
                0.1209298849,
                0.9961068541,
                0.9852396640,
                1.0,
                1.0,
                1.0,
                0.5945958284,
                0.5945958284,
                0.5819195021,
            ],
            [
                1.50586962, 4.07652330, 3.08938554, 1.66787669, 2.12496461, 0.88131236, 0.77893604,
                4.96177039,
            ],
            [
                1.20178441, 1.77679972, 3.43158409, 0.73223306, 2.82889966, 0.99284001, 1.57725564,
                2.38840681,
            ],
        ),
        (
            "/CONDITION=MATRIX /CRITERIA=MAXITER(300)",
            [
                0.0303245546,
                0.0149004829,
                2.0351390491,
                0.0002220244,
                0.0144656162,
                0.0286932776,
                0.0156963423,
                0.0208680030,
                0.9997779756,
                0.9991778767,
                0.9861111111,
                0.9833333333,
                0.9722222222,
                0.5945958284,
                0.5841999121,
                0.5837609304,
            ],
            [
                1.40374507, 4.33734758, 4.14737525, 1.64953861, 1.72164848, 1.39413999, 1.20535503,
                4.63329107,
            ],
            [
                1.31555334, 1.81376276, 3.41898888, 0.72520354, 2.41446636, 1.16303044, 1.88969690,
                2.36118631,
            ],
        ),
    ];
    for (options, measures, first, second) in cases {
        let tables = run(
            JUDGES,
            &format!(
                "PREFSCAL i1 TO i4 /INPUT=SOURCES(2) /MODEL=WEIGHTED {options} \
                 /PRINT=MEASURES INDIVIDUAL DISTANCES WEIGHTS."
            ),
        );
        check(
            &tables,
            &measures,
            &[
                ("Distances: Source 1", &first),
                ("Distances: Source 2", &second),
            ],
        );
        assert!(tables.iter().any(|t| t.title == "Dimension Weights"));
    }
}

fn measures_of(tables: &[Table]) -> Vec<f64> {
    table(tables, "Measures").rows[1..]
        .iter()
        .map(|r| number(&r[1]))
        .collect()
}

#[test]
fn identifier_variables_place_the_cases_like_counts_do() {
    // JUDGES with row and source numbers, sorted by row and then source
    let mut lines: Vec<(usize, usize, &str)> = JUDGES
        .lines()
        .skip(2)
        .take(12)
        .enumerate()
        .map(|(c, line)| (c % 6 + 1, c / 6 + 1, line))
        .collect();
    lines.sort();
    let body: String = lines
        .iter()
        .map(|(r, s, line)| format!("{line} {r} {s}\n"))
        .collect();
    let ids = format!(
        "DATA LIST FREE /i1 TO i4 person judge.\nBEGIN DATA.\n{body}END DATA.\n\
         VALUE LABELS judge 1 'first' 2 'second'.\n"
    );
    let options = "/MODEL=WEIGHTED /CONDITION=MATRIX /CRITERIA=MAXITER(50) /PRINT=MEASURES";
    let by_count = measures_of(&run(
        JUDGES,
        &format!("PREFSCAL i1 TO i4 /INPUT=SOURCES(2) {options}."),
    ));
    let by_rows = measures_of(&run(
        JUDGES,
        &format!("PREFSCAL i1 TO i4 /INPUT=ROWS(6) {options}."),
    ));
    let tables = run(
        &ids,
        &format!("PREFSCAL i1 TO i4 /INPUT=ROWS(person) SOURCES(judge) {options} WEIGHTS."),
    );
    assert_eq!(by_count, by_rows);
    for (a, b) in by_count.iter().zip(measures_of(&tables)) {
        close(b, *a, "identifiers");
    }
    let weights = table(&tables, "Dimension Weights");
    assert_eq!(weights.rows.len(), 2);
    assert_eq!(weights.rows[1][0], Cell::Text("second".into()));
}

#[test]
fn every_start_lowers_the_penalized_stress() {
    for start in [
        "CLASSICAL(SPEARMAN)",
        "ROSSCLIFF",
        "CORRESPONDENCE",
        "CENTROIDS(2)",
        "RANDOM(3)",
    ] {
        let tables = run(
            TASTE,
            &format!(
                "SET SEED=7.\nPREFSCAL c1 TO c5 /INITIAL={start} /CONDITION=UNCONDITIONAL \
                 /PRINT=HISTORY MULTIPLE INITIAL MEASURES."
            ),
        );
        let history = &table(&tables, "Iteration History").rows;
        let first = number(&history[0][1]);
        let last = number(&history[history.len() - 1][1]);
        assert!(
            last.is_finite() && last < first,
            "{start}: {first} to {last}"
        );
        for pair in history.windows(2) {
            assert!(
                number(&pair[1][1]) <= number(&pair[0][1]) + 1e-12,
                "{start}"
            );
        }
        assert_eq!(table(&tables, "Initial Common Space").rows.len(), 13);
        if start.starts_with("RANDOM") {
            assert_eq!(table(&tables, "Random Starts").rows.len(), 3);
        }
    }
}

#[test]
fn files_charts_and_several_dimensionalities() {
    let mut engine = Engine::default();
    let source = format!(
        "{TASTE}COMPUTE grp = MOD($CASENUM, 2).\nDATASET NAME taste.\nDATASET DECLARE space.\n\
         DATASET DECLARE dist.\n\
         PREFSCAL c1 TO c5 /CRITERIA=DIMENSIONS(1,3) MAXITER(100) /OPTIONS=COLORS(grp) \
         /PRINT=DECOMPOSITION TRANSFORMATION \
         /PLOT=COMMON STRESS TRANSFORMATIONS(1 TO 3)(8) SHEPARD(2) FIT RESIDUALS \
         /OUTFILE=COMMON(space) DISTANCES(dist).\n"
    );
    let (commands, errors) = parse(&source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut tables = Vec::new();
    for command in &commands {
        tables.extend(engine.execute(&command.command).unwrap());
    }
    // the decomposition adds up to the normalized stress
    let total = table(&tables, "Decomposition of Normalized Stress (1 Dimensions)")
        .rows
        .last()
        .map(|r| number(&r[2]))
        .unwrap();
    assert!(total > 0.0 && total < 1.0);
    assert!(
        tables
            .iter()
            .any(|t| t.title == "Transformed Proximities (3 Dimensions)")
    );
    let charts = engine.take_charts();
    let titles: Vec<&str> = charts.iter().map(|c| c.title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "Scree Plot",
            "Common Space",
            "Transformation Plot",
            "Transformation Plot",
            "Shepard Plot",
            "Fit Plot",
            "Residuals Plot"
        ]
    );
    let (commands, _) = parse("DATASET ACTIVATE space.\n");
    engine.execute(&commands[0].command).unwrap();
    let data = engine.dataset().unwrap();
    assert_eq!(data.case_count(), 13);
    assert_eq!(data.variable_names().collect::<Vec<_>>(), ["DIM_1"]);
    let (commands, _) = parse("DATASET ACTIVATE dist.\n");
    engine.execute(&commands[0].command).unwrap();
    let data = engine.dataset().unwrap();
    assert_eq!(data.case_count(), 13);
    assert_eq!(data.variable_names().count(), 13);
}

#[test]
fn problems_are_reported() {
    let fails = |data: &str, syntax: &str| {
        let (commands, errors) = parse(&format!("{data}{syntax}"));
        assert!(errors.is_empty(), "{errors:?}");
        let mut engine = Engine::default();
        let mut last = None;
        for command in &commands {
            last = engine.execute(&command.command).err();
        }
        last.map(|e| e.to_string()).unwrap_or_default()
    };
    assert!(fails(TASTE, "PREFSCAL c1 TO c5 /INPUT=SOURCES(3).").contains("divide"));
    assert!(fails(TASTE, "PREFSCAL c1.").contains("two variables"));
    assert!(fails(TASTE, "PREFSCAL c1 TO c5 /WEIGHTS=c1 c2.").contains("WEIGHTS"));
    let mut engine = Engine::default();
    let (commands, _) = parse(&format!(
        "{TASTE}PREFSCAL c1 TO c5 /MODEL=WEIGHTED /CRITERIA=MAXITER(5)."
    ));
    for command in &commands {
        engine.execute(&command.command).unwrap();
    }
    assert!(
        engine
            .warnings()
            .iter()
            .any(|w| w.contains("identity model"))
    );
}
