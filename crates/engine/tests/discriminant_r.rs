//! DISCRIMINANT against R on 60 invented cases (groups of 18, 22 and 20, four predictors, equal
//! priors): the eigenvalues of W^-1 B, canonical correlations and Wilks' lambda tests written
//! out in base R from the manual's formulas, the canonical coefficients scaled to unit pooled
//! within-groups variance (first coefficient positive), their standardized form, the structure
//! matrix, the group centroids and Fisher's classification functions; and the classification
//! table against `MASS::lda` with equal priors, whose rule is the same (its own scalings use
//! prior-weighted means, so its coefficients differ when the groups are of unequal size).
//!
//! The second part runs the classification rule of every kind the first leaves out on the same
//! 60 cases against R: the groups' own sizes as priors (`MASS::lda`), the separate covariance
//! matrices of the groups (the manual's rule on the canonical functions, written out in base R),
//! and leave-one-out (`MASS::lda` with `CV = TRUE`). The saved
//! predicted groups of all 60 cases and the saved posterior probabilities of six of them are
//! compared, to 1e-9. The manual says cross-validation applies to linear discrimination only, so
//! with SEPARATE the cross-validated table is that of the pooled rule, as `lda` gives it.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /g x1 x2 x3 x4.
BEGIN DATA
1 3.63 10.95 0.98 7.84
1 5.3 11.14 2.65 7.95
1 4.52 10.47 3.7 9.47
1 4.8 9.9 1.58 6.79
1 5.07 10.55 3.01 9.14
1 4.6 11.42 2.11 8.33
1 5.04 10.91 2.32 6.62
1 6.07 11.77 2.83 7.59
1 7.24 9.83 1.2 5.03
1 5.28 11.55 3.67 6.17
1 5.23 9.75 2.66 7.42
1 4.82 8.88 2.09 7.52
1 5.51 10.35 1.91 5.83
1 5.8 11.36 3.13 6.1
1 5.07 10.71 3.92 6.95
1 5.45 10.9 4.01 6.93
1 4.82 10.25 3.56 7.95
1 4.47 9.67 1.02 9.14
2 4.5 10.58 5.02 9.43
2 5.18 9.69 4.22 9.38
2 6.11 10.49 3.08 7.68
2 5.28 10.73 2.6 6.82
2 6.93 10.84 2.67 8.47
2 6.03 9.15 2.94 6.47
2 7.11 11.36 3.53 6.53
2 7.06 11.57 2.01 8.31
2 4.88 9.84 3.12 7.4
2 5.38 10.63 0.72 7.68
2 6.02 9.04 2.92 7.89
2 7.15 9.91 2.82 8.68
2 7.36 9.64 3.96 7.15
2 6.45 10.83 2.07 5.79
2 6.69 11.23 3.69 10.42
2 7.81 11.68 2.7 6.18
2 6.78 11.49 4.07 5.84
2 4.73 10.6 4.7 6.37
2 3.57 12.26 2.92 6.73
2 7.41 10.76 3.48 4.97
2 8.52 10.32 3.14 5.92
2 7.26 10.93 3.91 7.5
3 4.56 8.31 4.06 7.53
3 6.44 9.58 4.91 7.54
3 7.13 8.65 3.92 6.19
3 5.02 8.94 3.55 8.76
3 5.83 10.92 3.6 7.95
3 4.39 7.49 5.28 5.24
3 5.87 9.28 4.5 8.57
3 5.75 9.59 3.04 8.08
3 7.33 11.88 7.16 7.42
3 6.96 8.95 4.2 6.61
3 4.97 10.7 5.69 10.32
3 4.67 9.32 4.45 7.75
3 5.29 8.79 6.06 9.07
3 5.81 7.33 5.22 7.16
3 4.91 9.55 2.96 7.15
3 6.49 9.85 3.23 6.09
3 5.7 10.22 3.35 8.08
3 5.68 8.22 2.41 8.04
3 7 8.83 4.61 9.01
3 5.14 10.73 3.09 6.99
END DATA.
";

fn run() -> Vec<Table> {
    let syntax = "DISCRIMINANT /GROUPS=g(1,3) /VARIABLES=x1 x2 x3 x4 /PRIORS=EQUAL \
                  /STATISTICS=COEFF RAW TABLE.";
    let (commands, errors) = parse(&format!("{DATA}{syntax}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        tables.extend(engine.execute(&command.command).expect("runs"));
    }
    tables
}

fn near(got: f64, want: f64, what: &str) {
    assert!(
        (got - want).abs() <= 1e-9 * want.abs().max(1e-3),
        "{what}: {got} vs {want}"
    );
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

/// The numbers of the table's rows from the column `first` on, in the order of the rows.
fn matrix(tables: &[Table], title: &str, first: usize) -> Vec<Vec<f64>> {
    table(tables, title)
        .rows
        .iter()
        .map(|r| r[first..].iter().map(number).collect())
        .collect()
}

fn compare(got: &[Vec<f64>], want: &[&[f64]], what: &str) {
    assert_eq!(got.len(), want.len(), "{what}");
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        for (j, want) in w.iter().enumerate() {
            near(g[j], *want, &format!("{what} row {i} column {j}"));
        }
    }
}

#[test]
fn eigenvalues_and_wilks_tests_agree_with_r() {
    let t = run();
    compare(
        &matrix(&t, "Eigenvalues", 1),
        &[
            &[0.960903364948, 80.145368882, 80.145368882, 0.700022116324],
            &[0.23804721492, 19.854631118, 100.0, 0.438493286567],
        ],
        "eigenvalues",
    );
    compare(
        &matrix(&t, "Wilks' Lambda", 1),
        &[
            &[0.41191404537, 49.2252021423, 8.0, 5.75606899637e-08],
            &[0.807723637636, 11.8512097936, 3.0, 0.00791062723188],
        ],
        "Wilks",
    );
}

#[test]
fn coefficients_structure_and_centroids_agree_with_r() {
    let t = run();
    compare(
        &matrix(&t, "Canonical Discriminant Function Coefficients", 1),
        &[
            &[0.2090538204993, 0.9577420094862],
            &[-0.7910737857936, 0.2971728983747],
            &[0.7022755293319, 0.0573389318617],
            &[0.0857961112874, 0.1376853395801],
            &[3.84744921473, -9.76827574618],
        ],
        "raw coefficients",
    );
    compare(
        &matrix(
            &t,
            "Standardized Canonical Discriminant Function Coefficients",
            1,
        ),
        &[
            &[0.208732628725, 0.9562705278599],
            &[-0.735914054384, 0.2764517247611],
            &[0.737892490248, 0.0602469621287],
            &[0.107885065852, 0.1731336269747],
        ],
        "standardized coefficients",
    );
    // the structure matrix is ordered by the size of the loadings on the functions
    compare(
        &matrix(&t, "Structure Matrix", 1),
        &[
            &[0.677490009272, 0.1408575082024],
            &[-0.628903320984, 0.3946244751480],
            &[0.117352884043, -0.0876041556031],
            &[0.117881874562, 0.9386322649267],
        ],
        "structure",
    );
    compare(
        &matrix(&t, "Functions at Group Centroids", 1),
        &[
            &[-1.000534960090, -0.528841630513],
            &[-0.363883472299, 0.598173265391],
            &[1.300753283610, -0.182033124469],
        ],
        "centroids",
    );
}

#[test]
fn the_classification_functions_and_table_agree_with_r() {
    let t = run();
    compare(
        &matrix(&t, "Classification Function Coefficients", 1),
        &[
            &[6.111225596241, 7.323709533214, 6.92447177114],
            &[11.222484188261, 11.053764168710, 9.50505747404],
            &[0.202431857257, 0.714158448169, 1.83845590604],
            &[5.789586355741, 5.999382006293, 6.03477838493],
            &[-97.7933629663, -105.957610353, -92.5491701817],
        ],
        "classification functions",
    );
    // MASS::lda with equal priors puts 15 3 0, 4 12 6 and 2 4 14 cases of the groups in each
    let results = table(&t, "Classification Results");
    let counts: Vec<Vec<f64>> = results
        .rows
        .iter()
        .take(3)
        .map(|r| r[3..].iter().map(number).collect())
        .collect();
    assert_eq!(
        counts,
        [
            vec![15.0, 3.0, 0.0, 18.0],
            vec![4.0, 12.0, 6.0, 22.0],
            vec![2.0, 4.0, 14.0, 20.0]
        ]
    );
}

// ---------------------------------------------------------------- the other classification rules

fn run_engine(options: &str) -> (Vec<Table>, Engine) {
    let syntax = format!("DISCRIMINANT /GROUPS=g(1,3) /VARIABLES=x1 x2 x3 x4 {options}.");
    let (commands, errors) = parse(&format!("{DATA}{syntax}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        tables.extend(engine.execute(&command.command).expect("runs"));
    }
    (tables, engine)
}

type Posteriors = [(usize, [f64; 3]); 6];

/// The saved groups of all cases and the saved probabilities of the six cases R gives.
fn check_saved(options: &str, classes: &str, posteriors: &Posteriors) {
    let (_, engine) = run_engine(options);
    let data = engine.dataset().unwrap();
    let column = |name: &str| -> Vec<f64> {
        let v = data.variable(name).unwrap().numeric().unwrap();
        (0..data.case_count()).map(|k| v.get(k).unwrap()).collect()
    };
    let predicted: String = column("DSC_1")
        .iter()
        .map(|g| char::from(b'0' + *g as u8))
        .collect();
    assert_eq!(predicted, classes, "{options}");
    let probabilities = [column("DSC1_1"), column("DSC2_1"), column("DSC3_1")];
    for (case, want) in posteriors {
        for j in 0..3 {
            near(
                probabilities[j][case - 1],
                want[j],
                &format!("{options} case {case} group {}", j + 1),
            );
        }
    }
}

#[test]
fn priors_from_the_group_sizes_agree_with_r() {
    check_saved(
        "/PRIORS=EQUAL /SAVE=CLASS PROBS",
        "111111122111111211332123221132322223122233332332333333122331",
        &[
            (1, [0.968861894445, 0.030325748219, 0.000812357336]),
            (2, [0.629153362873, 0.347478362354, 0.023368274773]),
            (25, [0.145056417120, 0.807010918162, 0.047932664719]),
            (41, [0.039253459055, 0.026843582528, 0.933902958417]),
            (50, [0.014928727921, 0.148990862233, 0.836080409846]),
            (60, [0.622568115100, 0.310788944608, 0.066642940292]),
        ],
    );
    // 18, 22 and 20 cases: the priors 0.3, 0.3667 and 0.3333
    check_saved(
        "/PRIORS=SIZE /SAVE=CLASS PROBS",
        "111111122111121211332123221132322223122233332332333333122331",
        &[
            (1, [0.962290110154, 0.036813393044, 0.000896496801]),
            (2, [0.582649815288, 0.393304606102, 0.024045578610]),
            (25, [0.122445444121, 0.832597822370, 0.044956733509]),
            (41, [0.035372009377, 0.029564630122, 0.935063360501]),
            (50, [0.013258114023, 0.161721870543, 0.825020015434]),
            (60, [0.578342828318, 0.352869577485, 0.068787594198]),
        ],
    );
}

#[test]
fn separate_group_covariances_follow_the_manuals_rule_in_r() {
    // The manual classifies from the canonical functions, with each group's own covariance
    // matrix of the function values (a quadratic rule in the two-dimensional function space),
    // not from all four variables as MASS::qda does, so qda is no oracle here: its groups
    // differ in 5 of the 60 cases. The rule is written out in base R: the functions from
    // W^-1 B scaled to V'WV = I, the group covariance of their values, the chi-square distance,
    // and the posterior proportional to prior |S_j|^(-1/2) exp(-d/2).
    check_saved(
        "/PRIORS=EQUAL /SAVE=CLASS PROBS /CLASSIFY=SEPARATE",
        "111111122111111111332123221132322223122233332333233333122331",
        &[
            (1, [0.790861651613, 0.201554999716, 0.007583348671]),
            (2, [0.733367924888, 0.232801149987, 0.033830925125]),
            (25, [0.037100276254, 0.859709806106, 0.103189917640]),
            (41, [0.001305239135, 0.039444199952, 0.959250560913]),
            (50, [0.001414203375, 0.158712378004, 0.839873418621]),
            (60, [0.739322028989, 0.201737352433, 0.058940618577]),
        ],
    );
    check_saved(
        "/PRIORS=SIZE /SAVE=CLASS PROBS /CLASSIFY=SEPARATE",
        "111111122111111111332123221132322223122233332332233333122331",
        &[
            (1, [0.756347550731, 0.235594224024, 0.008058225245]),
            (2, [0.694811065838, 0.269575309303, 0.035613624859]),
            (25, [0.030852308343, 0.873801077373, 0.095346614285]),
            (41, [0.001170252002, 0.043223768984, 0.955605979014]),
            (50, [0.001253072437, 0.171879975170, 0.826866952393]),
            (60, [0.703192370390, 0.234518439628, 0.062289189982]),
        ],
    );
}

#[test]
fn leave_one_out_tables_agree_with_lda_in_r() {
    let cross = |options: &str| -> Vec<Vec<f64>> {
        let (tables, _) = run_engine(&format!("{options} /STATISTICS=TABLE CROSSVALID"));
        table(&tables, "Classification Results")
            .rows
            .iter()
            .skip(6)
            .take(3)
            .map(|r| r[3..].iter().map(number).collect())
            .collect()
    };
    let equal = [
        vec![15.0, 3.0, 0.0, 18.0],
        vec![5.0, 11.0, 6.0, 22.0],
        vec![2.0, 5.0, 13.0, 20.0],
    ];
    let size = [
        vec![13.0, 5.0, 0.0, 18.0],
        vec![4.0, 12.0, 6.0, 22.0],
        vec![2.0, 5.0, 13.0, 20.0],
    ];
    assert_eq!(cross("/PRIORS=EQUAL"), equal);
    assert_eq!(cross("/PRIORS=SIZE"), size);
    // cross-validation is linear discrimination only: SEPARATE leaves it as it is
    assert_eq!(cross("/PRIORS=EQUAL /CLASSIFY=SEPARATE"), equal);
    assert_eq!(cross("/PRIORS=SIZE /CLASSIFY=SEPARATE"), size);
}

#[test]
fn a_block_subcommand_after_a_global_one_is_ignored_with_a_warning() {
    // found by this audit: SAVE written after CLASSIFY was dropped without a word
    let (_, engine) = run_engine("/CLASSIFY=SEPARATE /SAVE=CLASS PROBS /PRIORS=SIZE");
    assert!(engine.dataset().unwrap().variable("DSC_1").is_err());
    let warnings = engine.warnings();
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("SAVE comes after a global subcommand")),
        "{warnings:?}"
    );
    assert!(warnings.iter().any(|w| w.contains("PRIORS comes after")));
    // in the order the manual gives, nothing is ignored
    let (_, engine) = run_engine("/PRIORS=SIZE /SAVE=CLASS PROBS /CLASSIFY=SEPARATE");
    assert!(engine.warnings().is_empty(), "{:?}", engine.warnings());
    assert!(engine.dataset().unwrap().variable("DSC_1").is_ok());
}
