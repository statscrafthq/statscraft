//! FACTOR against R on 80 invented cases of six variables with two factors: KMO and Bartlett's
//! test (`psych::KMO`, `cortest.bartlett`), principal components (`principal`, also with
//! `stats::varimax` and Kaiser normalization), principal axis and ULS factoring (the fixed
//! point of the communalities iterated to 1e-14 in base R, which `psych::fa` with
//! `fm = "minres"` reproduces to 1e-5), ML factoring (the discrepancy function minimized to
//! full precision, `factanal` agrees on the chi-square to 1e-9), and direct oblimin with
//! `GPArotation::oblimin`. The columns are compared with the signs R gives them (a positive
//! column sum). ML is compared to 2e-3 only: the manual's Newton steps treat a variable whose
//! second derivative is below 0.05 as a Heywood variable (here v1, with a unique variance of
//! 0.06), which leaves the loadings up to 1.4e-3 and the communalities 7e-4 from the exact optimum
//! (the discrepancy is within 2e-7 of its minimum).
//!
//! The second part covers the options the first leaves out: GLS (the discrepancy 1/2 tr((I -
//! R^-1 S)^2) with S = L L' + diag(psi), minimized to full precision over L and psi from
//! several starts, chi-square (N - 1 - (2p + 5)/6 - 2m/3) F; `psych::fa` stops earlier and is
//! no use as an oracle there) and alpha factoring (the fixed point of Kaiser and Caffrey in
//! base R, which `psych::fa(fm = "alpha", min.err = 1e-14)` reproduces to 1e-9), quartimax and
//! equamax of three principal components with Kaiser normalization (`GPArotation::cfT` with
//! kappa 0 and m / (2 p)), promax with kappa 4 (the steps of `stats::promax` with the target
//! taken from the row-normalized varimax loadings, as the Hendrickson-White method has it:
//! `stats::promax` itself powers the loadings without normalizing them and differs by up to
//! 0.015 here), and the three kinds of scores
//! (regression, Bartlett, Anderson-Rubin) from the manual's matrix formulas written out in
//! base R on the standardized data. Rotated columns may come in another order or sign than R's
//! and are matched before they are compared.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /v1 v2 v3 v4 v5 v6.
BEGIN DATA
-0.63 -0.73 -0.85 1.01 0.49 0.83
0.74 0.52 1.25 -0.26 -0.31 1.58
0.28 0.83 1.07 -0.2 -0.11 0.47
0.37 0.95 -0.45 0.22 -0.12 -1.11
-0.6 0.64 0.09 -0.65 -0.84 0.14
0.89 -0.38 1.84 0.23 -0.27 -0.45
-1.27 -1.11 -0.89 1.1 0.32 1.03
-0.56 -0.96 0.45 0.52 0.86 -0.31
-0.21 0.15 -0.38 0.2 0.35 -0.79
0.34 0.34 1.41 -1.8 -1.52 -0.96
-2.62 -1.22 -1.43 -0.32 0.42 -0.06
0.24 0.84 0.89 0.54 1.56 0.93
-0.25 0.56 0.47 0.46 0.75 0.2
-0.59 -0.28 0.37 -2.06 -2.13 -2.95
-0.17 -0.09 -1.05 -1.83 -0.98 -0.07
-0.2 0.2 0.78 0.66 0.53 0.25
0.68 0.39 -0.32 0.41 -1.1 -0.19
0.94 1.14 1.56 1.39 0.95 0.09
-1.24 0.68 0.16 2.73 2.16 2.09
0 1.09 -1.1 -0.42 -0.15 0.43
-0.31 0.09 -0.05 -0.35 0.81 -0.37
1.84 1.35 -0.13 -0.92 -1.31 -0.66
0.09 1.44 1 1.04 -0.42 1.25
-0.79 -0.91 0.01 0.41 -0.57 -2.24
-1.13 -0.28 -0.17 -1.32 -1.74 0.5
0.98 -0.14 1.17 -0.37 -0.22 0.46
0.21 0.69 -0.87 -0.28 0.17 -0.21
-1.03 -0.98 -0.28 1.52 0.65 2.35
1.59 1.15 -0.7 -0.08 -0.11 -0.68
0.91 0.32 0.63 -0.13 -0.5 1.73
-1.55 -1.83 -0.24 0.76 1.49 0.84
-0.8 -1.39 -0.21 -0.07 0.86 2.26
0.09 1.2 -1.49 -0.72 -0.68 -1.16
-0.87 0.11 -0.75 0.11 -1.45 -1.34
0.53 1.41 -0.89 0.41 -0.16 -0.61
-0.48 0.08 -0.23 -0.53 0.05 0.13
-0.6 -0.2 0.22 -0.05 -1.24 -0.11
0.26 0.42 -0.74 0.81 0.4 0.39
-0.12 -0.3 -0.1 -0.29 0.46 -0.29
1.03 0.81 0.86 1 1.47 -0.01
-0.53 -0.55 -0.94 0.77 0.71 0.07
-0.58 -1.13 -0.73 0.82 1.51 0.73
-0.34 -2.23 -1.55 -0.48 -0.39 -0.86
-0.55 0.08 0.38 0.47 1.33 1.48
0.27 1.64 0.31 -0.12 -2.14 -1.03
0.76 0.73 -0.68 -0.01 0.02 -0.75
-0.41 -0.96 1.04 1.17 0.52 -1.38
1.17 0.93 0.61 0.75 0.93 0.31
0.81 0.72 2.03 0.32 0.05 0.99
0.59 1.17 0.39 0.91 1.12 0.6
-1.58 -0.67 -1.53 -0.55 -2.07 -0.44
0.35 0.73 0.71 0.13 -0.13 -0.72
2.15 1.65 1.48 -1.52 -0.43 0.8
1.24 0.71 -0.52 0.41 -0.03 -0.26
-2.16 -0.45 -2.82 -1.1 -1.11 0.76
-0.97 -0.45 -0.03 -0.36 -0.24 0.13
-0.2 0.79 1.14 1.21 1.32 0.98
-0.3 0 -0.65 0.37 0.96 0.54
0.75 1.47 1.07 1.24 1.78 -0.29
-1.6 -0.43 -1.08 -1.63 -1.84 0.59
1.1 -0.2 0.63 -0.41 -0.75 -0.26
-0.52 1.25 -0.91 -1.29 -1.1 -1.88
-0.17 -0.24 1.65 -0.11 -0.78 -0.2
0.06 0.74 0.86 0.36 1.32 -0.73
-0.37 -0.07 -0.19 -0.07 -0.49 0.9
-0.28 -0.47 -0.35 -1.39 -0.91 -0.67
-1.09 -0.19 -0.34 -0.21 1.21 -0.03
-0.88 0.33 -0.46 -1.15 -1.18 -1.39
0.05 -0.14 0.7 -0.61 -0.07 -0.04
0.95 1.13 0.48 1.81 1.14 0.29
-1.95 -0.59 -1.31 0.76 1.83 0.39
0.55 -0.13 -0.96 -0.06 -0.6 -0.89
0.81 0.13 1.46 0.86 0.66 0.08
-0.16 0 -0.02 1.15 0.84 0.01
0.47 -0.64 -0.42 0.23 1.9 1.03
1.02 1.97 2.14 1.65 1.36 2.82
-0.29 -0.32 0.23 0.33 -0.31 0.5
-0.37 1.16 -0.35 -1.2 -1.64 -0.44
-2.02 -1.73 -1.52 -0.61 -1.04 0.44
1.31 1.48 1.83 0.35 1.69 0.8
END DATA.
";

fn run(syntax: &str) -> Vec<Table> {
    let (commands, errors) = parse(&format!("{DATA}{syntax}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        tables.extend(engine.execute(&command.command).expect("runs"));
    }
    tables
}

fn near(got: f64, want: f64, tolerance: f64, what: &str) {
    assert!((got - want).abs() <= tolerance, "{what}: {got} vs {want}");
}

fn table<'a>(tables: &'a [Table], title: &str) -> &'a Table {
    tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"))
}

fn number(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        other => panic!("{other:?} is no number"),
    }
}

/// The numbers of the six variables' rows, from the column `first` on.
fn matrix(tables: &[Table], title: &str, first: usize) -> Vec<Vec<f64>> {
    table(tables, title)
        .rows
        .iter()
        .map(|r| r[first..].iter().map(number).collect())
        .collect()
}

fn compare(got: &[Vec<f64>], want: &[[f64; 2]], tolerance: f64, what: &str) {
    assert_eq!(got.len(), want.len(), "{what}");
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        for j in 0..2 {
            near(
                g[j],
                w[j],
                tolerance,
                &format!("{what} v{} factor {}", i + 1, j + 1),
            );
        }
    }
}

const PC: [[f64; 2]; 6] = [
    [0.60676762, 0.67353104],
    [0.51720280, 0.66373343],
    [0.70802956, 0.35281908],
    [0.72585044, -0.48574634],
    [0.71178380, -0.53526057],
    [0.52061932, -0.51515365],
];
const PAF: [[f64; 2]; 6] = [
    [0.662229132682, 0.703436726429],
    [0.451856460803, 0.507200041156],
    [0.588624207278, 0.252235936173],
    [0.675966662369, -0.473416538436],
    [0.695901067422, -0.548463093569],
    [0.388894244847, -0.364624159288],
];

#[test]
fn kmo_bartlett_and_principal_components_agree_with_r() {
    let t = run(
        "FACTOR VARIABLES=v1 TO v6 /PRINT=INITIAL EXTRACTION KMO /CRITERIA=FACTORS(2) \
                 /EXTRACTION=PC /ROTATION=NOROTATE.",
    );
    let kmo = table(&t, "KMO and Bartlett's Test");
    near(number(&kmo.rows[0][2]), 0.6454319345, 1e-9, "KMO");
    near(
        number(&kmo.rows[1][2]),
        162.4482564,
        1e-6,
        "Bartlett chi-square",
    );
    near(number(&kmo.rows[2][2]), 15.0, 1e-12, "df");
    compare(&matrix(&t, "Component Matrix", 1), &PC, 1e-7, "components");
    let communalities: Vec<f64> = table(&t, "Communalities")
        .rows
        .iter()
        .map(|r| number(&r[2]))
        .collect();
    for (got, want) in communalities.iter().zip([
        0.8218110005,
        0.7080407949,
        0.62578716,
        0.7628083726,
        0.7931400559,
        0.5364277637,
    ]) {
        near(*got, want, 1e-8, "communality");
    }
}

#[test]
fn principal_axis_and_uls_agree_with_r() {
    for method in ["PAF", "ULS"] {
        let t = run(&format!(
            "FACTOR VARIABLES=v1 TO v6 /PRINT=EXTRACTION /CRITERIA=FACTORS(2) ITERATE(1000) \
             ECONVERGE(1E-12) /EXTRACTION={method} /ROTATION=NOROTATE."
        ));
        compare(&matrix(&t, "Factor Matrix", 1), &PAF, 1e-7, method);
        let communalities: Vec<f64> = table(&t, "Communalities")
            .rows
            .iter()
            .map(|r| number(&r[1]))
            .collect();
        for (got, want) in communalities.iter().zip([
            0.933370652261,
            0.461426142919,
            0.410101424891,
            0.681054147499,
            0.785090060647,
            0.284189511211,
        ]) {
            near(*got, want, 1e-8, method);
        }
    }
}

#[test]
fn ml_agrees_with_r_to_the_accuracy_of_the_manuals_steps() {
    let t = run(
        "FACTOR VARIABLES=v1 TO v6 /PRINT=EXTRACTION /CRITERIA=FACTORS(2) ITERATE(1000) \
                 ECONVERGE(1E-12) /EXTRACTION=ML /ROTATION=NOROTATE.",
    );
    compare(
        &matrix(&t, "Factor Matrix", 1),
        &[
            [0.9655848159746, -0.0807546236281],
            [0.6705602896900, -0.0811256523176],
            [0.6047101651877, 0.1874610961234],
            [0.1798564969751, 0.8152222937459],
            [0.1718764139686, 0.8675409556966],
            [0.0246405096653, 0.5305652675807],
        ],
        2e-3,
        "ML",
    );
    let fit = table(&t, "Goodness-of-fit Test");
    near(number(&fit.rows[0][0]), 2.27922209619, 1e-4, "chi-square");
    near(number(&fit.rows[0][1]), 4.0, 1e-12, "df");
    near(number(&fit.rows[0][2]), 0.6845545157, 1e-4, "p");
}

#[test]
fn varimax_and_oblimin_agree_with_r() {
    let t = run(
        "FACTOR VARIABLES=v1 TO v6 /PRINT=EXTRACTION ROTATION /CRITERIA=FACTORS(2) \
                 /EXTRACTION=PC /ROTATION=VARIMAX.",
    );
    compare(
        &matrix(&t, "Rotated Component Matrix", 1),
        &[
            [-0.00359867611882, 0.9065307771986],
            [-0.06331758740702, 0.8390659557337],
            [0.28697075181405, 0.7371804037196],
            [0.86390449119089, 0.1283643357232],
            [0.88677768508402, 0.0822520097211],
            [0.73173989979062, -0.0313764676898],
        ],
        1e-6,
        "PC varimax",
    );
    let t = run(
        "FACTOR VARIABLES=v1 TO v6 /PRINT=EXTRACTION ROTATION /CRITERIA=FACTORS(2) \
                 ITERATE(1000) ECONVERGE(1E-12) /EXTRACTION=PAF /ROTATION=VARIMAX.",
    );
    compare(
        &matrix(&t, "Rotated Factor Matrix", 1),
        &[
            [-0.00444715664216, 0.96610086174193],
            [-0.02178813887033, 0.67893403208500],
            [0.25298147335627, 0.58830417220168],
            [0.81613181007277, 0.12240513096589],
            [0.88227551621001, 0.08173110878291],
            [0.53308254134779, 0.00353770003016],
        ],
        1e-6,
        "PAF varimax",
    );
    // direct oblimin, delta 0, with Kaiser normalization: the pattern and the factor correlation
    let t = run(
        "FACTOR VARIABLES=v1 TO v6 /PRINT=EXTRACTION ROTATION /CRITERIA=FACTORS(2) \
                 ITERATE(1000) ECONVERGE(1E-12) DELTA(0) /EXTRACTION=PAF /ROTATION=OBLIMIN.",
    );
    compare(
        &matrix(&t, "Pattern Matrix", 1),
        &[
            [-0.0852662820925, 0.97656577085667],
            [-0.0787812238962, 0.68787293118965],
            [0.2064643686774, 0.57296685926751],
            [0.8145079268357, 0.05438771826280],
            [0.8847500393796, 0.00767293020362],
            [0.5384106426519, -0.04168791444911],
        ],
        1e-5,
        "oblimin pattern",
    );
    let phi = matrix(&t, "Factor Correlation Matrix", 1);
    near(phi[0][1], 0.165611924887, 1e-5, "factor correlation");
}

// ---------------------------------------------------------------- options the first part leaves out

fn run_engine(syntax: &str) -> (Vec<Table>, Engine) {
    let (commands, errors) = parse(&format!("{DATA}{syntax}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        tables.extend(engine.execute(&command.command).expect("runs"));
    }
    (tables, engine)
}

/// Every column of `want` must equal one column of `got`, up to its sign, each column of `got`
/// used once. The columns are the columns of the row-per-variable matrices.
fn match_columns(got: &[Vec<f64>], want: &[&[f64]], tolerance: f64, what: &str) {
    let columns = want[0].len();
    let mut used = vec![false; columns];
    for j in 0..columns {
        let found = (0..columns).find(|&k| {
            !used[k]
                && [1.0, -1.0].iter().any(|sign| {
                    got.iter()
                        .zip(want)
                        .all(|(g, w)| (sign * g[k] - w[j]).abs() <= tolerance)
                })
        });
        let k =
            found.unwrap_or_else(|| panic!("{what}: column {} of R is not among {got:?}", j + 1));
        used[k] = true;
    }
}

#[test]
fn gls_and_alpha_agree_with_r() {
    let t = run(
        "FACTOR VARIABLES=v1 TO v6 /PRINT=EXTRACTION /CRITERIA=FACTORS(2) ITERATE(5000) \
         ECONVERGE(1E-13) /EXTRACTION=GLS /ROTATION=NOROTATE.",
    );
    // v1 has a unique variance of 0.033, below the 0.05 at which the manual's Newton steps call a
    // variable a Heywood variable, so the solution stops up to 1.3e-4 from the exact optimum
    let communalities: Vec<f64> = table(&t, "Communalities")
        .rows
        .iter()
        .map(|r| number(&r[1]))
        .collect();
    for (got, want) in communalities.iter().zip([
        0.9668133802,
        0.4439117070,
        0.3924652570,
        0.6888715735,
        0.7992795686,
        0.2827210777,
    ]) {
        near(*got, want, 2e-4, "GLS communality");
    }
    // the loadings are fixed up to a rotation: their cross products are not
    let loadings = matrix(&t, "Factor Matrix", 1);
    let cross =
        |i: usize, j: usize| loadings[i][0] * loadings[j][0] + loadings[i][1] * loadings[j][1];
    near(cross(0, 1), 0.6546895046, 2e-4, "GLS l1 l2");
    near(cross(0, 2), 0.5698876266, 2e-4, "GLS l1 l3");
    near(cross(3, 4), 0.7418747878, 2e-4, "GLS l4 l5");
    near(cross(4, 5), 0.4698875350, 2e-4, "GLS l5 l6");
    let fit = table(&t, "Goodness-of-fit Test");
    near(
        number(&fit.rows[0][0]),
        2.2622762599,
        1e-4,
        "GLS chi-square",
    );
    near(number(&fit.rows[0][1]), 4.0, 1e-12, "GLS df");
    near(number(&fit.rows[0][2]), 0.6876454146, 1e-4, "GLS p");

    let t = run(
        "FACTOR VARIABLES=v1 TO v6 /PRINT=EXTRACTION /CRITERIA=FACTORS(2) ITERATE(5000) \
         ECONVERGE(1E-13) /EXTRACTION=ALPHA /ROTATION=NOROTATE.",
    );
    compare(
        &matrix(&t, "Factor Matrix", 1),
        &[
            [0.581061362427, 0.773215805526],
            [0.398331068528, 0.552265542516],
            [0.577344007884, 0.297542145943],
            [0.707505640718, -0.392889980691],
            [0.734072161805, -0.482697508889],
            [0.435030535042, -0.322530730527],
        ],
        1e-7,
        "alpha",
    );
    let communalities: Vec<f64> = table(&t, "Communalities")
        .rows
        .iter()
        .map(|r| number(&r[1]))
        .collect();
    for (got, want) in communalities.iter().zip([
        0.935494988821,
        0.463664869606,
        0.421857432052,
        0.654926768575,
        0.771858823826,
        0.293277638554,
    ]) {
        near(*got, want, 1e-7, "alpha communality");
    }
}

#[test]
fn quartimax_and_equamax_agree_with_r() {
    let pc3: [(&str, &[&[f64]]); 2] = [
        (
            "QUARTIMAX",
            &[
                &[0.036172027856, 0.902898930033, -0.080423286364],
                &[-0.011656185699, 0.834573383912, -0.117698755607],
                &[0.174979332147, 0.749023974818, 0.283551533069],
                &[0.925485188951, 0.109435623545, 0.091618260914],
                &[0.913804325809, 0.067396700796, 0.163955264167],
                &[0.355444730404, 0.007463903735, 0.903993821964],
            ],
        ),
        (
            "EQUAMAX",
            &[
                &[0.057626593579, 0.903673558685, -0.055281764539],
                &[0.012377030679, 0.837056673007, -0.098410020612],
                &[0.159796639730, 0.739301551010, 0.316155664257],
                &[0.914676020672, 0.090849476664, 0.178905131842],
                &[0.895706405497, 0.047429377775, 0.248842277493],
                &[0.270525256180, -0.018956150860, 0.932769128184],
            ],
        ),
    ];
    for (method, want) in pc3 {
        let t = run(&format!(
            "FACTOR VARIABLES=v1 TO v6 /PRINT=ROTATION /CRITERIA=FACTORS(3) ITERATE(5000) \
             /EXTRACTION=PC /ROTATION={method}."
        ));
        match_columns(
            &matrix(&t, "Rotated Component Matrix", 1),
            want,
            1e-5,
            method,
        );
    }
}

#[test]
fn promax_agrees_with_r() {
    let cases: [(&str, [[f64; 2]; 6], f64); 2] = [
        (
            "PAF",
            [
                [-0.076246617581, 0.976078728573],
                [-0.072467668721, 0.687773813728],
                [0.212300420333, 0.569338947307],
                [0.816748063392, 0.043685447579],
                [0.886699058777, -0.003868169382],
                [0.539159963554, -0.048635337909],
            ],
            0.169118953497,
        ),
        (
            "PC",
            [
                [-0.070864785096, 0.914893565843],
                [-0.126159012800, 0.851866760753],
                [0.235080062283, 0.719525676375],
                [0.862756509613, 0.056627942795],
                [0.889271201412, 0.008176125186],
                [0.741158028120, -0.093383793780],
            ],
            0.156099705035,
        ),
    ];
    for (method, pattern, phi) in cases {
        let t = run(&format!(
            "FACTOR VARIABLES=v1 TO v6 /PRINT=ROTATION /CRITERIA=FACTORS(2) ITERATE(5000) \
             ECONVERGE(1E-13) KAISER /EXTRACTION={method} /ROTATION=PROMAX(4)."
        ));
        match_columns(
            &matrix(&t, "Pattern Matrix", 1),
            &pattern.iter().map(|r| &r[..]).collect::<Vec<_>>(),
            1e-6,
            method,
        );
        let correlation = table(
            &t,
            if method == "PC" {
                "Component Correlation Matrix"
            } else {
                "Factor Correlation Matrix"
            },
        );
        near(
            number(&correlation.rows[0][2]).abs(),
            phi,
            1e-6,
            "promax factor correlation",
        );
    }
}

#[test]
fn factor_scores_agree_with_r() {
    // cases 1 to 3 and the variances of the two score columns, PAF unrotated and varimax
    type Scores = ([[f64; 2]; 3], [f64; 2]);
    let cases: [(&str, &str, Scores); 6] = [
        (
            "REG",
            "NOROTATE",
            (
                [
                    [0.080900743225, -0.898495390901],
                    [0.499609570916, 0.804398939012],
                    [0.231479238841, 0.419944860390],
                ],
                [0.8999788037, 0.9030260481],
            ),
        ),
        (
            "REG",
            "VARIMAX",
            (
                [
                    [0.677535131555, -0.595638349363],
                    [-0.191881054472, 0.927280452382],
                    [-0.121448684476, 0.463882033304],
                ],
                [0.8614119646, 0.9415928873],
            ),
        ),
        (
            "BART",
            "NOROTATE",
            (
                [
                    [0.126655384428, -1.071758635427],
                    [0.465799419789, 0.841771814595],
                    [0.210597113939, 0.477741931137],
                ],
                [1.1144648545, 1.1116854488],
            ),
        ),
        (
            "BART",
            "VARIMAX",
            (
                [
                    [0.830049125211, -0.689729373562],
                    [-0.242134223528, 0.931085337184],
                    [-0.176397943652, 0.491398272933],
                ],
                [1.1616713964, 1.0644789068],
            ),
        ),
        (
            "AR",
            "NOROTATE",
            (
                [
                    [0.099315767271, -1.014459846853],
                    [0.457822347673, 0.809245039737],
                    [0.208870805146, 0.458094933803],
                ],
                [1.0, 1.0],
            ),
        ),
        (
            "AR",
            "VARIMAX",
            (
                [
                    [0.770761733981, -0.667022302428],
                    [-0.225512653718, 0.902010465228],
                    [-0.164116519776, 0.475966122276],
                ],
                [1.0, 1.0],
            ),
        ),
    ];
    for (method, rotation, (first, variances)) in cases {
        let (_, engine) = run_engine(&format!(
            "FACTOR VARIABLES=v1 TO v6 /CRITERIA=FACTORS(2) ITERATE(5000) ECONVERGE(1E-13) \
             /EXTRACTION=PAF /ROTATION={rotation} /SAVE={method}(ALL)."
        ));
        let data = engine.dataset().unwrap();
        let column = |name: &str| -> Vec<f64> {
            let v = data.variable(name).unwrap().numeric().unwrap();
            (0..data.case_count()).map(|k| v.get(k).unwrap()).collect()
        };
        let scores = [column("FAC1_1"), column("FAC2_1")];
        let n = scores[0].len() as f64;
        let variance = |c: &[f64]| {
            let mean = c.iter().sum::<f64>() / n;
            c.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0)
        };
        // a rotated factor may come out with the other sign or in the other place
        let mut used = [false; 2];
        for j in 0..2 {
            let found = (0..2).find(|&k| {
                !used[k]
                    && [1.0, -1.0].iter().any(|sign| {
                        (0..3).all(|c| (sign * scores[k][c] - first[c][j]).abs() <= 1e-6)
                    })
            });
            let k =
                found.unwrap_or_else(|| panic!("{method} {rotation}: column {} not found", j + 1));
            used[k] = true;
            near(
                variance(&scores[k]),
                variances[j],
                1e-8,
                &format!("{method} {rotation} variance"),
            );
        }
    }
}
