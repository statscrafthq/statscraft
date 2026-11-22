//! FACTOR and QUICK CLUSTER on synthetic data. The expected numbers come from an independent
//! computation in a general numerical library from the raw values: the correlation matrix,
//! KMO, Bartlett's test and the anti-image matrices by their definitions; principal components
//! from the eigen decomposition; PAF and alpha factoring by their fixed-point iterations; ML,
//! GLS and ULS by minimizing the discrepancy function over loadings and unique variances with a
//! general optimizer; orthogonal rotations by maximizing the orthomax criterion over all
//! rotations; oblimin by gradient projection (Bernaards and Jennrich); promax from the
//! varimax solution; k-means by Lloyd's algorithm. PSPP comparisons are in reference_factor.rs.

use statscraft_data::Values;
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

/// id, group, weight and six items with two factors; case 4 misses x6, case 18 misses x1.
const DATA: &str = "DATA LIST FREE /id g w x1 x2 x3 x4 x5 x6.
BEGIN DATA.
1 1 2 5 5 6 2 2 3
2 1 1 5 4 5 4 4 3
3 1 3 1 2 1 5 4 4
4 1 2 5 5 5 4 3 .
5 1 1 4 4 4 5 5 6
6 1 2 6 4 6 5 5 3
7 1 1 5 5 4 5 4 6
8 1 1 4 5 5 6 6 6
9 1 1 4 3 5 6 6 7
10 1 2 4 4 3 3 3 4
11 1 3 5 5 5 3 2 3
12 1 3 6 7 5 6 6 7
13 1 3 6 6 5 7 6 6
14 1 3 3 2 4 3 4 2
15 1 3 5 3 4 5 4 4
16 1 2 3 1 3 3 2 4
17 1 1 3 3 3 5 5 5
18 1 2 . 4 2 7 6 6
19 1 1 3 5 4 4 4 5
20 1 1 5 5 4 2 5 2
21 2 2 5 4 4 6 5 4
22 2 1 6 6 7 5 6 5
23 2 2 5 6 6 4 6 4
24 2 2 3 4 4 1 3 4
25 2 1 5 7 4 7 6 7
26 2 3 2 2 1 3 2 4
27 2 2 4 4 5 1 4 2
28 2 1 3 5 5 4 4 4
29 2 3 4 5 4 2 4 4
30 2 2 3 4 4 4 5 5
31 2 2 3 3 3 4 5 4
32 2 2 4 3 3 2 4 2
33 2 1 6 6 5 2 3 4
34 2 1 4 6 5 5 4 5
35 2 2 2 2 2 3 2 3
36 2 2 5 5 5 5 3 2
37 2 1 5 4 5 4 3 3
38 2 1 4 5 4 4 3 4
39 2 3 3 4 3 5 4 5
40 2 2 5 4 4 5 5 4
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

fn find<'t>(tables: &'t [Table], title: &str) -> &'t Table {
    tables.iter().find(|t| t.title == title).unwrap_or_else(|| {
        panic!(
            "no table {title} in {:?}",
            tables.iter().map(|t| &t.title).collect::<Vec<_>>()
        )
    })
}

fn number(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        other => panic!("not a number: {other:?}"),
    }
}

fn label(cell: &Cell) -> String {
    match cell {
        Cell::Text(t) => t.clone(),
        Cell::Number(n) => n.to_string(),
        Cell::Empty => String::new(),
    }
}

/// The numbers of the row whose last row header is `row` (the first such row).
fn row(table: &Table, row: &str) -> Vec<f64> {
    let h = table.row_headers;
    let r = table
        .rows
        .iter()
        .find(|r| h > 0 && label(&r[h - 1]) == row)
        .unwrap_or_else(|| panic!("{}: no row {row}", table.title));
    r[h..].iter().map(number).collect()
}

/// The rows of a matrix table by variable, in the order of `names`.
fn rows(table: &Table, names: &[&str]) -> Vec<Vec<f64>> {
    names.iter().map(|n| row(table, n)).collect()
}

const X: [&str; 6] = ["x1", "x2", "x3", "x4", "x5", "x6"];

fn assert_close(actual: &[Vec<f64>], expected: &[[f64; 2]], tolerance: f64, what: &str) {
    for (i, (a, e)) in actual.iter().zip(expected).enumerate() {
        for j in 0..2 {
            assert!(
                (a[j] - e[j]).abs() <= tolerance,
                "{what} [{i}][{j}]: ours {}, expected {}",
                a[j],
                e[j]
            );
        }
    }
}

fn close(a: f64, b: f64, tolerance: f64) -> bool {
    (a - b).abs() <= tolerance * b.abs().max(1.0)
}

#[test]
fn principal_components_with_varimax_kmo_and_scores() {
    let mut engine = Engine::default();
    let tables = run_on(
        &mut engine,
        &format!("{DATA}FACTOR VARIABLES=x1 TO x6 /PRINT=ALL /CRITERIA=FACTORS(2) /SAVE=REG(ALL)."),
    );
    let d = find(&tables, "Descriptive Statistics");
    assert!(close(row(d, "x1")[1], 1.2417580343, 1e-9));
    assert_eq!(row(d, "x1")[2], 38.0);
    let kmo = find(&tables, "KMO and Bartlett's Test");
    assert!(close(number(&kmo.rows[0][2]), 0.6966339492534586, 1e-12));
    assert!(close(number(&kmo.rows[1][2]), 111.5280988530234, 1e-10));
    assert_eq!(number(&kmo.rows[2][2]), 15.0);
    assert!(number(&kmo.rows[3][2]) < 1e-15);
    let corr = find(&tables, "Correlation Matrix");
    assert!(close(row(corr, "x1")[1], 0.6774173203, 1e-9));
    assert!(corr.caption.as_deref().unwrap().contains("0.038"));
    let aic = find(&tables, "Anti-image Matrices");
    let anti = rows(aic, &X);
    assert!(close(anti[0][1], -0.1383107626, 1e-9));
    // the second block holds the correlations with the MSA on the diagonal
    let msa = [
        0.6776048968,
        0.7346923484,
        0.7176942425,
        0.6383486097,
        0.8180136225,
        0.607201259,
    ];
    for (i, msa) in msa.iter().enumerate() {
        assert!(close(number(&aic.rows[6 + i][2 + i]), *msa, 1e-9));
    }
    assert!(close(number(&aic.rows[6][3]), -0.3920658630, 1e-9));
    let tve = find(&tables, "Total Variance Explained");
    let eigen = [
        3.0918421344,
        1.6357096428,
        0.4928591878,
        0.3671023541,
        0.2338181013,
        0.1786685796,
    ];
    for (k, e) in eigen.iter().enumerate() {
        assert!(close(number(&tve.rows[k][1]), *e, 1e-9));
    }
    assert!(close(
        number(&tve.rows[1][9]),
        100.0 * (eigen[0] + eigen[1]) / 6.0,
        1e-9
    ));
    assert!(close(number(&tve.rows[0][7]), 2.449914983, 1e-8));
    let pc = rows(find(&tables, "Component Matrix"), &X);
    let expected_pc = [
        [0.7488474771, -0.5278188462],
        [0.8040056375, -0.3133812829],
        [0.7106625552, -0.58793942],
        [0.6637959451, 0.600459172],
        [0.7548692807, 0.3588279287],
        [0.607577625, 0.6510972422],
    ];
    assert_close(&pc, &expected_pc, 1e-9, "component matrix");
    let rotated = rows(find(&tables, "Rotated Component Matrix"), &X);
    let expected_varimax = [
        [0.9104147428, 0.1025196291],
        [0.8092820853, 0.2994919033],
        [0.9217790062, 0.0322101335],
        [0.0976836272, 0.8897382665],
        [0.3262189672, 0.7695234231],
        [0.0220237077, 0.8902770049],
    ];
    assert_close(&rotated, &expected_varimax, 1e-6, "varimax");
    let t = find(&tables, "Component Transformation Matrix");
    assert!(close(number(&t.rows[0][1]), 0.7477673989, 1e-6));
    assert!(close(number(&t.rows[1][1]), -0.6639607798, 1e-6));
    let coefficients = rows(find(&tables, "Component Score Coefficient Matrix"), &X);
    assert!(close(coefficients[0][0], 0.3953602006, 1e-6));
    assert!(close(coefficients[5][1], 0.4281250675, 1e-6));
    let data = engine.dataset().unwrap();
    let scores = |name: &str| match data.variable(name).unwrap().values() {
        Values::Numeric(v) => (0..data.case_count()).map(|k| v.get(k)).collect::<Vec<_>>(),
        Values::String(_) => panic!(),
    };
    let (f1, f2) = (scores("FAC1_1"), scores("FAC2_1"));
    assert!(close(f1[0].unwrap(), 1.1690861752, 1e-6));
    assert!(close(f2[2].unwrap(), 0.6012730613, 1e-6));
    // a case missing a variable gets no score
    assert_eq!(f1[3], None);
    let label = data.variable("FAC2_1").unwrap().label().map(str::to_string);
    assert_eq!(label.as_deref(), Some("REGR factor score 2 for analysis 1"));
}

#[test]
fn unrotated_scores_and_orthogonal_rotations() {
    let tables = run(
        "FACTOR VARIABLES=x1 TO x6 /PRINT=EXTRACTION FSCORE /CRITERIA=FACTORS(2) /EXTRACTION=PC.",
    );
    // EXTRACTION without ROTATION: no rotation
    assert!(tables.iter().all(|t| !t.title.starts_with("Rotated")));
    let w = rows(find(&tables, "Component Score Coefficient Matrix"), &X);
    assert!(close(w[0][0], 0.2422010712, 1e-9) && close(w[0][1], -0.3226849267, 1e-9));
    let quartimax = run("FACTOR VARIABLES=x1 TO x6 /CRITERIA=FACTORS(2) /ROTATION=QUARTIMAX.");
    let q = rows(find(&quartimax, "Rotated Component Matrix"), &X);
    assert_close(
        &q,
        &[
            [0.9106700865, 0.1002261035],
            [0.8100338939, 0.297452491],
            [0.9218572146, 0.029888206],
            [0.0999244367, 0.8894893933],
            [0.3281562483, 0.7686992845],
            [0.0242661142, 0.8902187062],
        ],
        1e-5,
        "quartimax",
    );
    // three components: the default criterion settles the loadings at the optimum
    let three = run("FACTOR VARIABLES=x1 TO x6 /CRITERIA=MINEIGEN(0.4) /ROTATION=VARIMAX.");
    let v = rows(find(&three, "Rotated Component Matrix"), &X);
    let optimum = [
        [0.9030339194, 0.1799655924, -0.0153207194],
        [0.8031200678, 0.0695349489, 0.5056805755],
        [0.9170497178, 0.1332721626, -0.0713784929],
        [0.055638989, 0.8304299742, 0.3618390659],
        [0.2844389033, 0.8723843714, 0.0962076156],
        [-0.0097777657, 0.5445528149, 0.7946355253],
    ];
    for (a, e) in v.iter().zip(optimum) {
        for j in 0..3 {
            assert!((a[j] - e[j]).abs() < 2e-4, "{a:?} {e:?}");
        }
    }
    // with two factors equamax is varimax
    let equamax = run("FACTOR VARIABLES=x1 TO x6 /CRITERIA=FACTORS(2) /ROTATION=EQUAMAX.");
    let e = rows(find(&equamax, "Rotated Component Matrix"), &X);
    assert!(close(e[0][0], 0.9104147428, 1e-5) && close(e[5][1], 0.8902770049, 1e-5));
}

const PAF: [[f64; 2]; 6] = [
    [0.740941221, -0.4696259486],
    [0.7476121175, -0.2346238604],
    [0.7102311145, -0.5392371775],
    [0.6253860609, 0.5683629805],
    [0.6773643947, 0.3142533824],
    [0.5621281925, 0.601367363],
];

#[test]
fn principal_axis_factoring_and_score_methods() {
    let tables = run(
        "FACTOR VARIABLES=x1 TO x6 /PRINT=INITIAL EXTRACTION ROTATION FSCORE /CRITERIA=FACTORS(2) \
         /EXTRACTION=PAF /ROTATION=VARIMAX /SAVE=BART(2 bart).",
    );
    let communalities = rows(find(&tables, "Communalities"), &X);
    assert!(close(communalities[0][0], 0.6774092361, 1e-9));
    assert!(close(communalities[0][1], 0.7695424246, 1e-9));
    let f = find(&tables, "Factor Matrix");
    assert_close(&rows(f, &X), &PAF, 1e-9, "PAF");
    assert!(
        f.caption
            .as_deref()
            .unwrap()
            .contains("10 iterations required")
    );
    let v = rows(find(&tables, "Rotated Factor Matrix"), &X);
    assert!(close(v[0][0], 0.8690657802, 1e-5) && close(v[4][1], 0.6771983201, 1e-5));
    let w = rows(find(&tables, "Factor Score Coefficient Matrix"), &X);
    let bartlett = [
        [0.4477707721, -0.0739628923],
        [0.1983774749, 0.0614771911],
        [0.5330707001, -0.1541986975],
        [-0.0838552448, 0.5271005968],
        [0.0197339911, 0.2523364168],
        [-0.0991543034, 0.468149466],
    ];
    assert_close(&w, &bartlett, 1e-5, "Bartlett");
    let ar = run(
        "FACTOR VARIABLES=x1 TO x6 /PRINT=FSCORE /CRITERIA=FACTORS(2) /EXTRACTION=PAF \
         /ROTATION=VARIMAX /SAVE=AR(ALL).",
    );
    let w = rows(find(&ar, "Factor Score Coefficient Matrix"), &X);
    assert!(close(w[0][0], 0.4212082067, 1e-5) && close(w[5][1], 0.4283505634, 1e-5));
    let reg = run(
        "FACTOR VARIABLES=x1 TO x6 /PRINT=FSCORE /CRITERIA=FACTORS(2) /EXTRACTION=PAF /ROTATION=VARIMAX.",
    );
    let w = rows(find(&reg, "Factor Score Coefficient Matrix"), &X);
    assert!(close(w[0][0], 0.4169338212, 1e-5) && close(w[5][1], 0.3831846349, 1e-5));
}

#[test]
fn least_squares_and_maximum_likelihood() {
    type Case = (&'static str, [[f64; 2]; 6], Option<f64>);
    let cases: [Case; 3] = [
        (
            "ML",
            [
                [0.8184503818, -0.3073190063],
                [0.7844040047, -0.0762728748],
                [0.8009351532, -0.3865384085],
                [0.4994960913, 0.6686284498],
                [0.6053106978, 0.4403650844],
                [0.423053803, 0.7187497683],
            ],
            Some(6.736624447388661),
        ),
        (
            "GLS",
            [
                [0.7987729292, -0.4119217394],
                [0.7901387801, -0.1665131126],
                [0.7402305853, -0.457183844],
                [0.57846539, 0.6265724703],
                [0.6384935879, 0.3693213807],
                [0.5094147324, 0.6951034927],
            ],
            Some(5.589614703598192),
        ),
        (
            "ULS",
            [
                [0.7403622395, -0.4685913221],
                [0.7474513766, -0.2344245404],
                [0.7111396872, -0.54041629],
                [0.6256484531, 0.5690465652],
                [0.6773119751, 0.3142684823],
                [0.5618310929, 0.6008546557],
            ],
            None,
        ),
    ];
    for (method, expected, chi) in cases {
        let tables = run(&format!(
            "FACTOR VARIABLES=x1 TO x6 /CRITERIA=FACTORS(2) /EXTRACTION={method}."
        ));
        assert_close(
            &rows(find(&tables, "Factor Matrix"), &X),
            &expected,
            2e-4,
            method,
        );
        let fit = tables.iter().find(|t| t.title == "Goodness-of-fit Test");
        match chi {
            Some(chi) => {
                let fit = fit.expect("ML and GLS test the fit");
                assert!(close(number(&fit.rows[0][0]), chi, 1e-3), "{method}");
                assert_eq!(number(&fit.rows[0][1]), 4.0);
            }
            None => assert!(fit.is_none()),
        }
    }
}

#[test]
fn alpha_and_image_factoring() {
    let alpha = run("FACTOR VARIABLES=x1 TO x6 /CRITERIA=FACTORS(2) /EXTRACTION=ALPHA.");
    assert_close(
        &rows(find(&alpha, "Factor Matrix"), &X),
        &[
            [0.7040786973, 0.5257292843],
            [0.7268365574, 0.2872731074],
            [0.6721306142, 0.5901733927],
            [0.6667055354, -0.530209587],
            [0.6955484894, -0.2648314049],
            [0.6062353881, -0.5483611943],
        ],
        1e-9,
        "alpha",
    );
    let image = run("FACTOR VARIABLES=x1 TO x6 /CRITERIA=FACTORS(2) /EXTRACTION=IMAGE.");
    assert_close(
        &rows(find(&image, "Factor Matrix"), &X),
        &[
            [0.7252497037, -0.3175043807],
            [0.734700882, -0.1323563924],
            [0.6918844111, -0.3671113886],
            [0.504515092, 0.528913937],
            [0.5930565822, 0.3422971411],
            [0.4468735385, 0.5684896703],
        ],
        1e-9,
        "image",
    );
}

#[test]
fn oblique_rotations() {
    // a tight criterion reaches the optimum of the independent computation
    let tables = run(
        "FACTOR VARIABLES=x1 TO x6 /CRITERIA=FACTORS(2) ITERATE(500) RCONVERGE(1E-12) /ROTATION=OBLIMIN.",
    );
    assert_close(
        &rows(find(&tables, "Pattern Matrix"), &X),
        &[
            [0.923424604, -0.0275045587],
            [0.7905004124, 0.1902877202],
            [0.9453777578, -0.1016278991],
            [-0.0289029157, 0.9026789091],
            [0.2241548089, 0.745358521],
            [-0.1069633468, 0.9143005112],
        ],
        1e-6,
        "oblimin pattern",
    );
    let structure = rows(find(&tables, "Structure Matrix"), &X);
    assert!(close(structure[1][1], 0.409777909, 1e-6));
    let phi = find(&tables, "Component Correlation Matrix");
    assert!(close(number(&phi.rows[0][2]), 0.2776598031, 1e-6));
    let tve = find(&tables, "Total Variance Explained");
    assert!(close(number(&tve.rows[0][7]), 2.6476670507, 1e-6));
    // the default criterion stops within display precision
    let tables = run("FACTOR VARIABLES=x1 TO x6 /CRITERIA=FACTORS(2) /ROTATION=OBLIMIN.");
    let pattern = rows(find(&tables, "Pattern Matrix"), &X);
    assert!(close(pattern[1][1], 0.1902877202, 2e-3));
    assert_eq!(tve.columns.len(), 8);

    let tables = run(
        "FACTOR VARIABLES=x1 TO x6 /CRITERIA=FACTORS(2) /EXTRACTION=PAF /ROTATION=OBLIMIN(-.5).",
    );
    let pattern = rows(find(&tables, "Pattern Matrix"), &X);
    assert!(close(pattern[0][0], 0.8762858818, 2e-3) && close(pattern[5][0], -0.0683356481, 2e-3));
    let phi = find(&tables, "Factor Correlation Matrix");
    assert!(close(number(&phi.rows[0][2]), 0.259997973, 2e-3));

    let tables = run("FACTOR VARIABLES=x1 TO x6 /CRITERIA=FACTORS(2) /ROTATION=PROMAX(4).");
    assert_close(
        &rows(find(&tables, "Pattern Matrix"), &X),
        &[
            [0.9279493946, -0.0412856207],
            [0.7909321748, 0.179517687],
            [0.9511926288, -0.1160895453],
            [-0.043559532, 0.9074427221],
            [0.2131489733, 0.7456261553],
            [-0.1221520903, 0.9202739493],
        ],
        1e-5,
        "promax pattern",
    );
    let phi = find(&tables, "Component Correlation Matrix");
    assert!(close(number(&phi.rows[0][2]), 0.305778021, 1e-5));
    // CRITERIA KAPPA gives the default power of PROMAX
    let again = run("FACTOR VARIABLES=x1 TO x6 /CRITERIA=FACTORS(2) KAPPA(4) /ROTATION=PROMAX.");
    assert_eq!(
        find(&again, "Pattern Matrix").rows,
        find(&tables, "Pattern Matrix").rows
    );
}

#[test]
fn missing_values_weights_and_select() {
    let eigen = |tables: &[Table]| -> Vec<f64> {
        find(tables, "Total Variance Explained")
            .rows
            .iter()
            .map(|r| number(&r[1]))
            .collect()
    };
    let pairwise =
        run("FACTOR VARIABLES=x1 TO x6 /MISSING=PAIRWISE /PRINT=UNIVARIATE INITIAL CORRELATION.");
    let expected = [
        2.9930535241,
        1.7684373954,
        0.4850222538,
        0.3572600548,
        0.2358724108,
        0.160354361,
    ];
    for (a, e) in eigen(&pairwise).iter().zip(expected) {
        assert!(close(*a, e, 1e-9));
    }
    let d = find(&pairwise, "Descriptive Statistics");
    assert_eq!(d.columns.last().unwrap(), "Missing N");
    let x1 = row(d, "x1");
    assert!(close(x1[0], 4.1794871795, 1e-9) && close(x1[1], 1.2327076503, 1e-9));
    assert_eq!(x1[2..], [39.0, 1.0]);
    let meansub = run("FACTOR VARIABLES=x1 TO x6 /MISSING=MEANSUB.");
    let expected = [
        2.9740884951,
        1.7621947395,
        0.4843229202,
        0.3585262979,
        0.2468371638,
        0.1740303835,
    ];
    for (a, e) in eigen(&meansub).iter().zip(expected) {
        assert!(close(*a, e, 1e-9));
    }
    // the weights are whole numbers: the same as the cases repeated
    let weighted = run("WEIGHT BY w.\nFACTOR VARIABLES=x1 TO x6 /PRINT=INITIAL KMO.");
    let expected = [
        3.2729815578,
        1.5539137997,
        0.4922693898,
        0.3666183839,
        0.174083749,
        0.1401331198,
    ];
    for (a, e) in eigen(&weighted).iter().zip(expected) {
        assert!(close(*a, e, 1e-9));
    }
    // SELECT: the matrix from group 1, scores for every case
    let mut engine = Engine::default();
    let selected = run_on(
        &mut engine,
        &format!("{DATA}FACTOR VARIABLES=x1 TO x6 /SELECT=g(1) /PRINT=UNIVARIATE /SAVE=REG(1)."),
    );
    assert_eq!(
        row(find(&selected, "Descriptive Statistics"), "x2")[2],
        18.0
    );
    let data = engine.dataset().unwrap();
    let Values::Numeric(scores) = data.variable("FAC1_1").unwrap().values() else {
        panic!()
    };
    assert!(scores.get(25).is_some() && scores.get(3).is_none());
}

#[test]
fn analysis_order_sort_blank_and_matrix_input() {
    // ANALYSIS: TO runs in the order of VARIABLES; LISTWISE still uses every VARIABLES variable
    let tables = run(
        "FACTOR VARIABLES=x6 x1 x2 x3 /ANALYSIS=x1 TO x3 /PRINT=UNIVARIATE EXTRACTION /FORMAT=SORT BLANK(.5).",
    );
    let d = find(&tables, "Descriptive Statistics");
    assert_eq!(d.rows.len(), 3);
    assert_eq!(row(d, "x1")[2], 38.0);
    let m = find(&tables, "Component Matrix");
    assert_eq!(m.columns.len(), 2);
    // a correlation matrix as input gives the same solution as the cases
    let cases = run(
        "FACTOR VARIABLES=x1 TO x6 /PRINT=EXTRACTION KMO /CRITERIA=FACTORS(2) /FORMAT=SORT BLANK(.4).",
    );
    let sorted = find(&cases, "Component Matrix");
    let order: Vec<String> = sorted.rows.iter().map(|r| label(&r[0])).collect();
    assert_eq!(order, ["x2", "x5", "x1", "x3", "x4", "x6"]);
    assert_eq!(sorted.rows[0][2], Cell::Empty);
    let kmo = number(&find(&cases, "KMO and Bartlett's Test").rows[1][2]);
    let mut matrix = String::from(
        "DATA LIST LIST /ROWTYPE_ (A8) VARNAME_ (A8) x1 x2 x3 x4 x5 x6.\nBEGIN DATA.\n",
    );
    matrix.push_str("N '' 38 38 38 38 38 38\n");
    let r = [
        [
            1.0,
            0.6774173203,
            0.7691873619,
            0.2462841254,
            0.339767113,
            0.0908995692,
        ],
        [
            0.6774173203,
            1.0,
            0.6582459591,
            0.2715487258,
            0.4042380774,
            0.3643649857,
        ],
        [
            0.7691873619,
            0.6582459591,
            1.0,
            0.1297684239,
            0.346066262,
            0.0559300632,
        ],
        [
            0.2462841254,
            0.2715487258,
            0.1297684239,
            1.0,
            0.6320189465,
            0.6854879275,
        ],
        [
            0.339767113,
            0.4042380774,
            0.346066262,
            0.6320189465,
            1.0,
            0.549980722,
        ],
        [
            0.0908995692,
            0.3643649857,
            0.0559300632,
            0.6854879275,
            0.549980722,
            1.0,
        ],
    ];
    for (i, row) in r.iter().enumerate() {
        let values: Vec<String> = row.iter().map(|v| v.to_string()).collect();
        matrix.push_str(&format!("CORR x{} {}\n", i + 1, values.join(" ")));
    }
    matrix.push_str(
        "END DATA.\nFACTOR MATRIX=IN(COR=*) /PRINT=EXTRACTION KMO /CRITERIA=FACTORS(2) /FORMAT=SORT BLANK(.4).\n",
    );
    let from_matrix = run_on(&mut Engine::default(), &matrix);
    let m = find(&from_matrix, "Component Matrix");
    for (a, b) in m.rows.iter().zip(&sorted.rows) {
        assert_eq!(label(&a[0]), label(&b[0]));
        for j in 1..3 {
            match (&a[j], &b[j]) {
                (Cell::Number(x), Cell::Number(y)) => assert!(close(*x, *y, 1e-8)),
                (x, y) => assert_eq!(x, y),
            }
        }
    }
    assert!(close(
        number(&find(&from_matrix, "KMO and Bartlett's Test").rows[1][2]),
        kmo,
        1e-7
    ));
    // a covariance matrix gives the correlations, the counts and the deviations
    let sd = [
        1.2417580343,
        1.4272296076,
        1.2703534975,
        1.5488719338,
        1.2845509425,
        1.4114447871,
    ];
    let mut matrix = String::from(
        "DATA LIST LIST /ROWTYPE_ (A8) VARNAME_ (A8) x1 x2 x3 x4 x5 x6.\nBEGIN DATA.\nN '' 38 38 38 38 38 38\n",
    );
    for (i, row) in r.iter().enumerate() {
        let values: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(j, v)| (v * sd[i] * sd[j]).to_string())
            .collect();
        matrix.push_str(&format!("COV x{} {}\n", i + 1, values.join(" ")));
    }
    matrix.push_str(
        "END DATA.\nFACTOR MATRIX=IN(COV=*) /PRINT=EXTRACTION COVARIANCE /CRITERIA=FACTORS(2) /FORMAT=SORT BLANK(.4).\n",
    );
    let from_cov = run_on(&mut Engine::default(), &matrix);
    for (a, b) in find(&from_cov, "Component Matrix")
        .rows
        .iter()
        .zip(&sorted.rows)
    {
        for j in 1..3 {
            match (&a[j], &b[j]) {
                (Cell::Number(x), Cell::Number(y)) => assert!(close(*x, *y, 1e-8)),
                (x, y) => assert_eq!(x, y),
            }
        }
    }
    let cov = find(&from_cov, "Covariance Matrix");
    assert!(close(row(cov, "x1")[0], sd[0] * sd[0], 1e-9));
}

#[test]
fn ignored_subcommands_and_one_factor_warn() {
    let mut engine = Engine::default();
    run_on(
        &mut engine,
        &format!("{DATA}FACTOR VARIABLES=x1 TO x3 /SAVE=REG /ROTATION=VARIMAX /PLOT=EIGEN."),
    );
    let warnings = engine.warnings();
    assert!(
        warnings.iter().any(|w| w.contains("SAVE is ignored")),
        "{warnings:?}"
    );
    assert!(
        warnings.iter().any(|w| w.contains("cannot be rotated")),
        "{warnings:?}"
    );
    // the scree plot of the three eigenvalues
    let charts = engine.take_charts();
    assert_eq!(charts.len(), 1);
    assert_eq!(charts[0].series[0].values.len(), 3);
}

#[test]
fn quick_cluster_kmeans() {
    let mut engine = Engine::default();
    let tables = run_on(
        &mut engine,
        &format!(
            "{DATA}QUICK CLUSTER x1 x2 x4 /CRITERIA=CLUSTER(3) MXITER(20) /INITIAL=(2 2 5 5 4 2 5 6 6) \
             /PRINT=INITIAL CLUSTER ID(id) DISTANCE ANOVA /SAVE=CLUSTER DISTANCE."
        ),
    );
    let history = find(&tables, "Iteration History");
    assert_eq!(history.rows.len(), 2);
    assert!(close(number(&history.rows[0][1]), 1.3787046262, 1e-9));
    let finals = find(&tables, "Final Cluster Centers");
    assert!(close(row(finals, "x1")[0], 2.9090909091, 1e-9));
    assert!(close(row(finals, "x4")[2], 5.4615384615, 1e-9));
    let counts = find(&tables, "Number of Cases in each Cluster");
    let n: Vec<f64> = counts.rows.iter().map(|r| number(&r[2])).collect();
    assert_eq!(n, [11.0, 15.0, 13.0, 39.0, 1.0]);
    let distances = find(&tables, "Distances between Final Cluster Centers");
    assert!(close(number(&distances.rows[0][2]), 2.7917298829, 1e-9));
    let anova = find(&tables, "ANOVA");
    assert!(close(row(anova, "x2")[4], 26.191096634093377, 1e-9));
    assert!(close(row(anova, "x2")[5], 9.525308918987704e-08, 1e-6));
    let membership = find(&tables, "Cluster Membership");
    // case 18 misses x1: it is left out, and the later cases keep their numbers
    assert_eq!(label(&membership.rows[17][0]), "19");
    assert!(close(number(&membership.rows[0][3]), 1.115546702, 1e-9));
    let data = engine.dataset().unwrap();
    let Values::Numeric(cluster) = data.variable("QCL_1").unwrap().values() else {
        panic!()
    };
    let Values::Numeric(distance) = data.variable("QCL_2").unwrap().values() else {
        panic!()
    };
    assert_eq!(cluster.get(2), Some(1.0));
    assert_eq!(cluster.get(17), None);
    assert!(close(distance.get(4).unwrap(), 1.7558018785, 1e-9));

    // weighted
    let tables = run(
        "WEIGHT BY w.\nQUICK CLUSTER x1 x2 x4 /CRITERIA=CLUSTER(3) MXITER(20) /INITIAL=(2 2 5 5 4 2 5 6 6).",
    );
    assert_eq!(find(&tables, "Iteration History").rows.len(), 3);
    let finals = find(&tables, "Final Cluster Centers");
    assert!(close(row(finals, "x2")[1], 4.5217391304, 1e-9));
    let counts = find(&tables, "Number of Cases in each Cluster");
    assert_eq!(number(&counts.rows[2][2]), 24.0);
}

#[test]
fn quick_cluster_initial_selection_classify_and_outfile() {
    // three tight groups: the initial pass must end with one center in each
    let data = "DATA LIST FREE /a b.\nBEGIN DATA.\n0 0 0.1 0 0 0.2 10 10 10.1 10 20 0 20.2 0 0.1 0.1 10 10.2\nEND DATA.\n";
    let mut engine = Engine::default();
    let tables = run_on(
        &mut engine,
        &format!(
            "{data}DATASET NAME cases.\nDATASET DECLARE centers.\nQUICK CLUSTER a b /CRITERIA=CLUSTER(3) /OUTFILE=centers /SAVE=CLUSTER(grp).\n\
             DATASET ACTIVATE centers.\nLIST."
        ),
    );
    let initial = find(&tables, "Initial Cluster Centers");
    let mut firsts: Vec<f64> = row(initial, "a");
    firsts.sort_by(f64::total_cmp);
    assert!(
        firsts[0] < 1.0 && (9.0..11.0).contains(&firsts[1]) && firsts[2] > 19.0,
        "{firsts:?}"
    );
    let list = find(&tables, "List");
    assert_eq!(list.columns[0], "CLUSTER_");
    assert_eq!(list.rows.len(), 3);
    // NOINITIAL keeps the first three cases; CLASSIFY does not move them
    let tables = run_on(
        &mut Engine::default(),
        &format!(
            "{data}QUICK CLUSTER a b /CRITERIA=CLUSTER(3) NOINITIAL /METHOD=CLASSIFY /PRINT=INITIAL."
        ),
    );
    assert_eq!(
        row(find(&tables, "Initial Cluster Centers"), "a"),
        [0.0, 0.1, 0.0]
    );
    assert!(tables.iter().all(|t| t.title != "Iteration History"));
}

#[test]
fn quick_cluster_update_file_and_iteration_limit() {
    let initial = "/INITIAL=(2 2 5 5 4 2 5 6 6)";
    let tables = run(&format!(
        "QUICK CLUSTER x1 x2 x4 /CRITERIA=CLUSTER(3) MXITER(30) /METHOD=KMEANS(UPDATE) {initial}."
    ));
    let history = find(&tables, "Iteration History");
    assert_eq!(history.rows.len(), 18);
    assert!(close(number(&history.rows[0][2]), 1.4037526641, 1e-9));
    let finals = find(&tables, "Final Cluster Centers");
    assert!(close(row(finals, "x1")[0], 2.5555555556, 1e-9));
    assert!(close(row(finals, "x4")[1], 2.875, 1e-9));

    // one pass only: a warning and the reason under the history
    let mut engine = Engine::default();
    let tables = run_on(
        &mut engine,
        &format!("{DATA}QUICK CLUSTER x1 x2 x4 /CRITERIA=CLUSTER(3) MXITER(1) {initial}."),
    );
    let caption = find(&tables, "Iteration History").caption.clone().unwrap();
    assert!(caption.starts_with("Iterations stopped"), "{caption}");
    assert!(
        engine
            .warnings()
            .iter()
            .any(|w| w.contains("did not converge"))
    );

    // CONVERGE stops when no center moves more than its share of the initial gap
    let tables = run(&format!(
        "QUICK CLUSTER x1 x2 x4 /CRITERIA=CLUSTER(3) CONVERGE(0.5) {initial}."
    ));
    assert_eq!(find(&tables, "Iteration History").rows.len(), 1);

    // FILE: the centers of an earlier run as the initial centers
    let tables = run(&format!(
        "DATASET NAME cases.\nDATASET DECLARE centers.\n\
         QUICK CLUSTER x1 x2 x4 /CRITERIA=CLUSTER(3) {initial} /OUTFILE=centers.\n\
         QUICK CLUSTER x1 x2 x4 /CRITERIA=CLUSTER(3) /FILE=centers /METHOD=CLASSIFY."
    ));
    let titled =
        |title: &str| -> Vec<&Table> { tables.iter().filter(|t| t.title == title).collect() };
    let (initials, finals) = (
        titled("Initial Cluster Centers"),
        titled("Final Cluster Centers"),
    );
    assert_eq!(initials[1].rows, finals[0].rows);
    assert_eq!(finals[1].rows, finals[0].rows);
}

// ---- METHOD=COVARIANCE ----
// Five items measured on very different scales (variances from 0.2 to 765), so a covariance
// analysis is far from the correlation analysis. Expected numbers from R on these values:
// eigen(cov(X)) for the components (loadings the eigenvectors times the root of the
// eigenvalues, rescaled loadings those divided by the standard deviations), the principal axis
// iteration written out on cov(X) with the diagonal replaced, varimax(normalize = TRUE) on the
// rescaled loadings, and the score coefficients by their formulas.

const SCALES: &str = "DATA LIST FREE /x1 x2 x3 x4 x5.
BEGIN DATA.
1.8 1.5 13 1.13 36\n\
-0.8 0.8 -5 0.06 -21\n\
-0.5 0 -2 -0.11 -32\n\
-0.6 -1.2 -5 -0.25 9\n\
-1.2 -0.5 5 0.29 -24\n\
-0.9 1.2 -6 -0.55 22\n\
0.3 2.6 14 0.26 14\n\
-0.3 -0.2 11 0.34 4\n\
-1.1 -2.8 4 -0.45 -33\n\
1.9 6.7 -8 0.76 55\n\
1.4 -0.5 -22 -0.09 -28\n\
1.4 7.4 4 -0.57 -26\n\
2 6.6 13 -0.1 6\n\
0.4 1.5 3 0.4 20\n\
1.9 6.4 11 -0.06 25\n\
-0.1 -0.6 6 0.69 0\n\
0.2 -0.9 -7 -0.6 -44\n\
-1.3 1.3 14 0.15 35\n\
-0.3 0.6 3 -0.3 2\n\
0.6 4.2 -13 -0.47 4\n\
1 -1.3 19 -0.04 -4\n\
0.9 -0.4 21 -0.39 -20\n\
0.9 3.1 14 0.52 78\n\
-1.8 -2.5 4 0.41 12\n\
1.1 3.7 7 -0.08 38\n\
-1 0.2 5 0.82 26\n\
1.2 -0.3 15 0.26 7\n\
1.1 2.3 13 0.13 23\n\
-0.1 -1.3 -5 0.3 -15\n\
-0.1 -2.9 0 -0.42 32\n\
-0.9 -2.6 -13 0.49 11\n\
0.8 3.8 2 -0.5 4\n\
0.5 -1.7 -8 -0.21 -10\n\
0.4 -0.6 -16 -0.39 -34\n\
-0.9 0.5 -15 -0.04 -23\n\
0.7 -2.9 -8 -0.98 26\n\
-0.2 -1.2 -15 -0.32 20\n\
0 0.2 -5 -0.19 -6\n\
0.6 2.3 -5 -0.02 26\n\
0.4 0.6 26 0.7 59
END DATA.
";

fn on_scales(syntax: &str) -> Vec<Table> {
    run_on(&mut Engine::default(), &format!("{SCALES}{syntax}"))
}

fn check(actual: &[f64], expected: &[f64], tolerance: f64, what: &str) {
    assert_eq!(actual.len(), expected.len(), "{what}: {actual:?}");
    for (a, e) in actual.iter().zip(expected) {
        assert!(
            (a - e).abs() <= tolerance * e.abs().max(1.0),
            "{what}: ours {actual:?}, expected {expected:?}"
        );
    }
}

#[test]
fn covariance_components_match_the_eigen_decomposition_in_r() {
    let tables = on_scales(
        "FACTOR VARIABLES=x1 TO x5 /METHOD=COVARIANCE /PRINT=INITIAL EXTRACTION \
         /CRITERIA=FACTORS(2) /ROTATION=NOROTATE.",
    );
    let loadings = find(&tables, "Component Matrix");
    assert_eq!(loadings.columns.len(), 5);
    let raw = [
        [0.235156458504, 0.1181199405036],
        [0.808446586847, 0.2727588705177],
        [4.852705633854, 10.3046960212127],
        [0.210020032111, 0.0858720235982],
        [27.598556309528, -1.8215437924875],
    ];
    let rescaled = [
        [0.238635927044, 0.1198676901491],
        [0.297521079721, 0.1003795612231],
        [0.426031847694, 0.9046764871160],
        [0.456029853211, 0.1864593863871],
        [0.997826236987, -0.0658579444367],
    ];
    for (i, name) in ["x1", "x2", "x3", "x4", "x5"].iter().enumerate() {
        let got = row(loadings, name);
        check(&got[..2], &raw[i], 1e-9, "raw loadings");
        check(&got[2..], &rescaled[i], 1e-9, "rescaled loadings");
    }
    // communalities: the variance of each item and its share
    let h = find(&tables, "Communalities");
    check(
        &row(h, "x3"),
        &[129.742948717949, 129.7355120584364, 1.0, 0.9999426815901],
        1e-9,
        "x3 communalities",
    );
    check(
        &row(h, "x1"),
        &[0.971051282051, 0.0692508803207, 1.0, 0.0713153688180],
        1e-9,
        "x1 communalities",
    );
    // the total variance: the initial eigenvalues are those of cov(X), a percent of their
    // sum; the extraction is a percent of that sum for the raw loadings and of 5 for the
    // rescaled ones
    let v = find(&tables, "Total Variance Explained");
    check(
        &row(v, "1"),
        &[
            785.982055196670,
            87.0111149142375,
            87.0111149142375,
            785.982055197,
            87.0111149142,
            87.0111149142,
            1.530589460044,
            30.6117892009,
            30.6117892009,
        ],
        1e-9,
        "variance explained, first",
    );
    check(
        &row(v, "2"),
        &[
            109.600505603774,
            12.1331805538006,
            99.1442954680381,
            109.600505604,
            12.1331805538,
            99.1442954680,
            0.881988237411,
            17.6397647482,
            48.2515539491,
        ],
        1e-9,
        "variance explained, second",
    );
}

#[test]
fn mineigen_counts_in_mean_variances() {
    // the mean variance is 180.66: one eigenvalue (785.98) is above it, 109.6 is not
    let tables = on_scales("FACTOR VARIABLES=x1 TO x5 /METHOD=COVARIANCE /ROTATION=NOROTATE.");
    assert_eq!(find(&tables, "Component Matrix").columns.len(), 3);
    // MINEIGEN(0.5) is half a mean variance, 90.33: two
    let tables = on_scales(
        "FACTOR VARIABLES=x1 TO x5 /METHOD=COVARIANCE /CRITERIA=MINEIGEN(0.5) /ROTATION=NOROTATE.",
    );
    assert_eq!(find(&tables, "Component Matrix").columns.len(), 5);
}

#[test]
fn covariance_principal_axis_matches_the_iteration_in_r() {
    let tables = on_scales(
        "FACTOR VARIABLES=x1 TO x5 /METHOD=COVARIANCE /PRINT=INITIAL EXTRACTION \
         /CRITERIA=FACTORS(2) ECONVERGE(0.0001) ITERATE(500) /EXTRACTION=PAF /ROTATION=NOROTATE.",
    );
    let loadings = find(&tables, "Factor Matrix");
    let raw = [
        [0.392215856955, 0.4601916035720],
        [1.329938062131, 2.2852898056865],
        [7.581811262829, -1.5627108463173],
        [0.343729057156, 0.0526851672131],
        [15.303648052777, 0.5626293092252],
    ];
    let rescaled = [
        [0.398019238856, 0.4670007816715],
        [0.489438157876, 0.8410226495180],
        [0.665627240737, -0.1371945136386],
        [0.746360763322, 0.1143986544002],
        [0.553303635792, 0.0203418715148],
    ];
    for (i, name) in ["x1", "x2", "x3", "x4", "x5"].iter().enumerate() {
        let got = row(loadings, name);
        check(&got[..2], &raw[i], 1e-8, "raw loadings");
        check(&got[2..], &rescaled[i], 1e-8, "rescaled loadings");
    }
    assert!(
        loadings
            .caption
            .as_deref()
            .unwrap()
            .contains("16 iterations")
    );
    let h = find(&tables, "Communalities");
    // initial: the squared multiple correlation times the variance, and itself; extraction
    check(
        &row(h, "x2"),
        &[
            2.8805029620545,
            6.991284745080,
            0.390122631792,
            0.946868807388,
        ],
        1e-8,
        "x2 communalities",
    );
    let v = find(&tables, "Total Variance Explained");
    // the raw extraction is a percent of the total variance, the rescaled one of 5
    let first = row(v, "1");
    assert!(
        (first[5] - 32.516577262357).abs() < 1e-7 * 100.0,
        "{first:?}"
    );
    assert!((first[8] - 34.0845590181).abs() < 1e-7 * 100.0, "{first:?}");
}

#[test]
fn covariance_image_factoring_is_the_correlation_analysis_scaled() {
    let cor = on_scales("FACTOR VARIABLES=x1 TO x5 /EXTRACTION=IMAGE /ROTATION=NOROTATE.");
    let cov = on_scales(
        "FACTOR VARIABLES=x1 TO x5 /METHOD=COVARIANCE /EXTRACTION=IMAGE /ROTATION=NOROTATE.",
    );
    let (a, b) = (find(&cor, "Factor Matrix"), find(&cov, "Factor Matrix"));
    let sd = [
        0.971051282051_f64.sqrt(),
        7.383583333333_f64.sqrt(),
        129.742948717949_f64.sqrt(),
        0.212097179487_f64.sqrt(),
        765.002564102564_f64.sqrt(),
    ];
    for (i, name) in ["x1", "x2", "x3", "x4", "x5"].iter().enumerate() {
        let want = row(a, name);
        let got = row(b, name);
        let k = want.len();
        check(
            &got[k..],
            &want,
            1e-9,
            "rescaled equals the correlation loadings",
        );
        let scaled: Vec<f64> = want.iter().map(|v| v * sd[i]).collect();
        check(
            &got[..k],
            &scaled,
            1e-9,
            "raw is the standard deviation times those",
        );
    }
}

#[test]
fn covariance_varimax_rotates_the_rescaled_loadings() {
    let tables = on_scales(
        "FACTOR VARIABLES=x1 TO x5 /METHOD=COVARIANCE /PRINT=EXTRACTION ROTATION \
         /CRITERIA=FACTORS(2) RCONVERGE(0.0000000001) ITERATE(1000) /ROTATION=VARIMAX.",
    );
    let rotated = find(&tables, "Rotated Component Matrix");
    let raw = [
        [0.208514880320, 0.160537923887],
        [0.742118221006, 0.421003362553],
        [2.812245571603, 11.037517243630],
        [0.189944203759, 0.124111312068],
        [27.443737560937, 3.440872105230],
    ];
    let rescaled = [
        [0.211600149467, 0.162913306895],
        [0.273111196196, 0.154935869643],
        [0.246894468249, 0.969012798232],
        [0.412437930266, 0.269490785500],
        [0.992228762699, 0.124404785026],
    ];
    for (i, name) in ["x1", "x2", "x3", "x4", "x5"].iter().enumerate() {
        let got = row(rotated, name);
        check(&got[..2], &raw[i], 2e-5, "raw rotated");
        check(&got[2..], &rescaled[i], 2e-5, "rescaled rotated");
    }
    let t = find(&tables, "Component Transformation Matrix");
    check(
        &row(t, "1"),
        &[0.981884280815, 0.189481553428],
        2e-5,
        "T row 1",
    );
    // the rotation sums of squares: raw as a percent of the total variance, rescaled of 5
    let v = find(&tables, "Total Variance Explained");
    let first = row(v, "1");
    check(
        &first[6..9],
        &[761.697753178, 84.3227530368, 84.3227530368],
        2e-5,
        "rotation raw",
    );
    check(
        &first[9..12],
        &[1.33494419104, 26.6988838208, 26.6988838208],
        2e-5,
        "rotation rescaled",
    );
}

#[test]
fn covariance_component_scores_are_those_of_the_raw_data() {
    let rows = |t: &Table| -> Vec<Vec<f64>> {
        ["x1", "x2", "x3", "x4", "x5"]
            .iter()
            .map(|n| row(t, n))
            .collect()
    };
    let plain = on_scales(
        "FACTOR VARIABLES=x1 TO x5 /METHOD=COVARIANCE /PRINT=FSCORE \
         /CRITERIA=FACTORS(2) /ROTATION=NOROTATE /SAVE=REG(ALL).",
    );
    let w = rows(find(&plain, "Component Score Coefficient Matrix"));
    let want = [
        [0.000294825716908, 0.001062017675508],
        [0.002794938727951, 0.006762385366482],
        [0.070325559982098, 1.070938445289255],
        [0.000123059610571, 0.000360833280136],
        [0.971192185339212, -0.459683065174019],
    ];
    for (a, e) in w.iter().zip(&want) {
        check(a, e, 1e-8, "unrotated coefficients");
    }
    let rotated = on_scales(
        "FACTOR VARIABLES=x1 TO x5 /METHOD=COVARIANCE /PRINT=FSCORE \
         /CRITERIA=FACTORS(2) RCONVERGE(0.0000000001) ITERATE(1000) /ROTATION=VARIMAX.",
    );
    let w = rows(find(&rotated, "Component Score Coefficient Matrix"));
    let want = [
        [8.82519780888e-05, 0.001098642496358],
        [1.46295911870e-03, 0.007169469224066],
        [-1.33871518353e-01, 1.064863021500544],
        [5.24590467738e-05, 0.000377614051936],
        [1.04069980171e+00, -0.267332571896198],
    ];
    for (a, e) in w.iter().zip(&want) {
        check(a, e, 2e-5, "rotated coefficients");
    }
}

#[test]
fn covariance_det_and_inverse_describe_the_covariance_matrix() {
    let tables = on_scales(
        "FACTOR VARIABLES=x1 TO x5 /METHOD=COVARIANCE /PRINT=DET INV /CRITERIA=FACTORS(1).",
    );
    let d = find(&tables, "Covariance Matrix");
    check(
        &[number(&d.rows[0][0])],
        &[54769.1555518],
        1e-9,
        "determinant",
    );
    let inv = find(&tables, "Inverse of Covariance Matrix");
    check(
        &[row(inv, "x1")[0], row(inv, "x3")[2], row(inv, "x5")[4]],
        &[1.67062464807521, 0.00974760570114, 0.00185643530656],
        1e-9,
        "inverse diagonal",
    );
}

#[test]
fn covariance_needs_a_method_that_works_on_covariances() {
    for method in ["ML", "GLS", "ULS", "ALPHA"] {
        let (commands, errors) = parse(&format!(
            "{SCALES}FACTOR VARIABLES=x1 TO x5 /METHOD=COVARIANCE /EXTRACTION={method}."
        ));
        assert!(errors.is_empty(), "{errors:?}");
        let mut engine = Engine::default();
        let mut failure = None;
        for c in &commands {
            if let Err(e) = engine.execute(&c.command) {
                failure = Some(e.to_string());
            }
        }
        let message = failure.unwrap_or_else(|| panic!("{method} ran"));
        assert!(
            message.contains("METHOD=COVARIANCE works with PC, PAF and IMAGE"),
            "{message}"
        );
    }
    // a correlation matrix with no standard deviations has no covariances
    let matrix = "DATA LIST LIST /ROWTYPE_ (A8) VARNAME_ (A8) a b.\nBEGIN DATA.\nN '' 30 30\n\
                  CORR a 1 0.4\nCORR b 0.4 1\nEND DATA.\n";
    let (commands, errors) = parse(&format!(
        "{matrix}FACTOR MATRIX=IN(COR=*) /METHOD=COVARIANCE."
    ));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut message = String::new();
    for c in &commands {
        if let Err(e) = engine.execute(&c.command) {
            message = e.to_string();
        }
    }
    assert!(message.contains("standard deviations"), "{message}");
}

// ---- MATRIX=OUT ----

fn texts_of(engine: &Engine, name: &str) -> Vec<String> {
    let data = engine.dataset().expect("data");
    match data.variable(name).expect("variable").values() {
        Values::String(v) => (0..data.case_count())
            .map(|k| v.get(k).trim().to_string())
            .collect(),
        Values::Numeric(_) => panic!("{name} is numeric"),
    }
}

fn numbers_of(engine: &Engine, name: &str) -> Vec<f64> {
    let data = engine.dataset().expect("data");
    match data.variable(name).expect("variable").values() {
        Values::Numeric(v) => (0..data.case_count())
            .map(|k| v.get(k).unwrap_or(f64::NAN))
            .collect(),
        Values::String(_) => panic!("{name} is a string"),
    }
}

#[test]
fn matrix_out_cor_holds_the_means_deviations_counts_and_correlations() {
    let mut engine = Engine::default();
    let tables = run_on(
        &mut engine,
        &format!(
            "{SCALES}FACTOR VARIABLES=x1 TO x5 /MATRIX=OUT(COR=*) /CRITERIA=FACTORS(2) /ROTATION=NOROTATE."
        ),
    );
    // the analysis is shown as usual
    assert!(tables.iter().any(|t| t.title == "Communalities"));
    // the active dataset is now the matrix: records MEAN, STDDEV, N and the correlations
    let kinds = texts_of(&engine, "ROWTYPE_");
    assert_eq!(
        kinds,
        [
            "MEAN", "STDDEV", "N", "CORR", "CORR", "CORR", "CORR", "CORR"
        ]
    );
    let names = texts_of(&engine, "VARNAME_");
    assert_eq!(&names[3..], ["x1", "x2", "x3", "x4", "x5"]);
    // R: colMeans, sd and cor of the 40 cases
    check(
        &[numbers_of(&engine, "x1")[0], numbers_of(&engine, "x5")[0]],
        &[0.2350, 6.85],
        1e-4,
        "means",
    );
    check(
        &[numbers_of(&engine, "x3")[1], numbers_of(&engine, "x5")[1]],
        &[11.3904762287601, 27.6586797245017],
        1e-4,
        "deviations",
    );
    assert_eq!(numbers_of(&engine, "x4")[2], 40.0);
    let x1 = numbers_of(&engine, "x1");
    check(
        &x1[3..],
        &[
            1.0,
            0.604063690101853,
            0.205333424704745,
            -0.003350437776528,
            0.228804358095609,
        ],
        1e-4,
        "correlations of x1",
    );
}

#[test]
fn matrix_out_cov_holds_the_covariances_and_reads_back_as_the_same_analysis() {
    let mut engine = Engine::default();
    let direct = run_on(
        &mut engine,
        &format!(
            "{SCALES}FACTOR VARIABLES=x1 TO x5 /METHOD=COVARIANCE /MATRIX=OUT(COV=*) /CRITERIA=FACTORS(2) /ROTATION=NOROTATE."
        ),
    );
    let kinds = texts_of(&engine, "ROWTYPE_");
    assert_eq!(kinds[..3], ["MEAN", "STDDEV", "N"]);
    assert!(kinds[3..].iter().all(|k| k == "COV"));
    let x1 = numbers_of(&engine, "x1");
    check(
        &x1[3..],
        &[
            0.971051282051282,
            1.617474358974359,
            2.304_743_589_743_59,
            -0.001520512820513,
            6.236153846153846,
        ],
        1e-4,
        "covariances of x1",
    );
    // the written matrix, read back, is the same covariance analysis
    let back = run_on(
        &mut engine,
        "FACTOR MATRIX=IN(COV=*) /METHOD=COVARIANCE /CRITERIA=FACTORS(2) /ROTATION=NOROTATE.",
    );
    let (a, b) = (
        find(&direct, "Component Matrix"),
        find(&back, "Component Matrix"),
    );
    for name in ["x1", "x2", "x3", "x4", "x5"] {
        check(
            &row(b, name),
            &row(a, name),
            1e-3,
            "loadings after the round trip",
        );
    }
}

#[test]
fn matrix_out_goes_to_a_file_and_leaves_the_data() {
    let path = std::env::temp_dir().join("statscraft_factor_matrix_out_test.sav");
    let _ = std::fs::remove_file(&path);
    let mut engine = Engine::default();
    run_on(
        &mut engine,
        &format!(
            "{SCALES}FACTOR VARIABLES=x1 TO x5 /MATRIX=OUT(COR='{}') /CRITERIA=FACTORS(1).",
            path.display().to_string().replace('\\', "/")
        ),
    );
    // the data stay active; the file holds the matrix
    assert!(engine.dataset().unwrap().variable("ROWTYPE_").is_err());
    assert!(path.exists());
    let back = run_on(
        &mut engine,
        &format!(
            "GET FILE='{}'.\nFACTOR MATRIX=IN(COR=*) /CRITERIA=FACTORS(1) /ROTATION=NOROTATE.",
            path.display().to_string().replace('\\', "/")
        ),
    );
    assert!(back.iter().any(|t| t.title == "Component Matrix"));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn matrix_out_cov_needs_the_deviations_of_a_correlation_matrix() {
    let matrix = "DATA LIST LIST /ROWTYPE_ (A8) VARNAME_ (A8) a b.\nBEGIN DATA.\nN '' 30 30\n\
                  CORR a 1 0.4\nCORR b 0.4 1\nEND DATA.\n";
    let (commands, errors) = parse(&format!("{matrix}FACTOR MATRIX=IN(COR=*) OUT(COV=*)."));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut message = String::new();
    for c in &commands {
        if let Err(e) = engine.execute(&c.command) {
            message = e.to_string();
        }
    }
    assert!(message.contains("standard deviations"), "{message}");
}
