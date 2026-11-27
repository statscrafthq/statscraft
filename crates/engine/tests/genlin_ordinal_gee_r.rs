//! GENLIN REPEATED with DISTRIBUTION=MULTINOMIAL and a working association between the
//! measurements (EXCHANGEABLE, AR(1), MDEPENDENT, UNSTRUCTURED) on 70 invented subjects with up to
//! four waves (some waves missing), a four-category response and three predictors, against an R
//! script that writes the manual's estimating equations with its own matrices: the stacked
//! indicators of the first three categories with their multinomial covariance, the association
//! blocks from the Pearson-like residuals, the generalized inverse of each subject's matrix and
//! Fisher scoring until the change is below 1e-13. No R package fits this association (multgee
//! models local odds ratios instead), so the script is the oracle, and the plain model of
//! independent measurements is checked against `MASS::polr` and `multgee::ordLORgee`. Scripts in
//! lab/oracles/gee-mult.
//!
//! Two conventions are on record because the manual leaves them open: a subject's matrix that is
//! not positive definite gets its own ridge, (R + cI) / (1 + c), that lifts its smallest eigenvalue
//! to 0.01, and the AR(1) block of two positions is the power of the estimated matrix by their
//! distance (a matrix power).

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /id wave arm stress y.
BEGIN DATA.
1 1 0 1.26 2
1 2 0 1.34 4
1 4 0 1.4 4
2 1 1 -0.15 4
2 2 1 0.36 3
2 3 1 0.65 4
2 4 1 0.59 4
3 1 0 0.86 3
3 2 0 0.39 2
3 3 0 0.42 2
3 4 0 0.23 4
4 1 0 0.7 4
4 2 0 0.51 4
4 3 0 0.6 4
4 4 0 0.69 4
5 1 1 1.97 1
5 2 1 1.44 3
5 3 1 1.97 3
5 4 1 1.49 4
6 1 1 0.22 4
6 2 1 0 4
6 3 1 -0.02 2
6 4 1 0.06 4
7 1 0 -1.01 1
7 2 0 -0.87 1
7 3 0 -0.63 1
7 4 0 -0.86 3
8 1 1 -0.95 1
8 2 1 -1.05 3
8 3 1 -1.44 2
8 4 1 -0.76 2
9 1 0 2.05 3
9 2 0 1.99 3
9 3 0 2.51 4
9 4 0 2.15 4
10 1 1 0.73 4
10 2 1 0.32 3
10 3 1 1.01 4
10 4 1 0.48 4
11 1 1 0.55 2
11 2 1 0.76 3
11 3 1 0.61 2
11 4 1 0.44 3
12 1 1 -0.17 2
12 3 1 -0.49 1
12 4 1 -0.7 1
13 1 1 1.56 4
13 2 1 2.13 4
13 3 1 1.41 2
13 4 1 1.95 3
14 1 1 -1.4 4
14 2 1 -1.71 2
14 3 1 -1.36 4
14 4 1 -1.65 2
15 1 1 -0.31 1
15 3 1 -0.27 2
15 4 1 -0.3 1
16 1 1 -1.69 4
16 2 1 -1.31 3
16 3 1 -0.77 4
16 4 1 -1.49 4
17 1 1 -0.84 2
17 2 1 -0.5 2
17 3 1 -1.27 4
17 4 1 -1.01 3
18 1 0 0.96 1
18 2 0 1.06 2
18 3 0 0.78 4
18 4 0 0.39 3
19 1 0 -0.03 3
19 2 0 -0.23 4
19 3 0 0.11 3
19 4 0 -0.43 2
20 1 0 -0.65 1
20 2 0 -1.16 2
20 3 0 -0.41 1
20 4 0 -1.17 1
21 1 1 -0.25 3
21 2 1 -1.12 3
21 4 1 -0.13 2
22 1 1 1.29 3
22 2 1 0.78 3
22 3 1 0.94 3
22 4 1 1 3
23 1 1 -0.35 2
23 2 1 -0.52 3
23 4 1 0.26 3
24 1 0 1.12 3
24 2 0 0.88 1
24 3 0 0.67 2
24 4 0 0.96 2
25 1 1 0.63 1
25 2 1 0.94 2
25 3 1 0.47 3
25 4 1 -0.01 3
26 1 1 1.04 3
26 3 1 1.48 4
26 4 1 1.41 4
27 1 0 -1.49 1
27 2 0 -1.16 2
27 3 0 -1.68 1
27 4 0 -1.8 2
28 1 0 -0.29 1
28 3 0 -0.59 2
28 4 0 -0.84 1
29 1 1 -0.91 3
29 2 1 -1.33 1
29 3 1 -1.28 2
29 4 1 -1.03 2
30 1 0 0.94 4
30 2 0 1.36 4
30 3 0 0.67 4
31 1 1 0.08 3
31 2 1 0.15 3
31 3 1 0.25 4
31 4 1 0.76 4
32 1 1 0.24 1
32 2 1 0.67 4
32 3 1 0.52 4
32 4 1 -0.15 1
33 1 1 -0.28 3
33 2 1 -0.09 1
33 3 1 -0.68 3
33 4 1 -0.19 4
34 1 1 -0.2 3
34 2 1 -0.52 3
34 3 1 -0.62 1
34 4 1 -0.64 1
35 1 1 1.22 4
35 2 1 1.5 4
35 3 1 1.09 4
35 4 1 1.27 4
36 1 1 0.08 1
36 2 1 -0.27 3
36 3 1 -0.57 2
36 4 1 -0.04 4
37 1 0 0.54 4
37 2 0 0.75 4
37 3 0 0.03 4
38 1 0 -0.8 1
38 2 0 -1.95 1
38 4 0 -0.67 4
39 1 0 0.06 4
39 2 0 0.83 4
39 3 0 0.85 4
39 4 0 0.11 4
40 1 0 0.11 3
40 2 0 0.08 3
40 3 0 -0.69 3
40 4 0 -0.24 4
41 1 0 -0.68 1
41 2 0 -0.9 1
41 3 0 -0.01 1
41 4 0 -0.4 1
42 1 1 1.29 1
42 2 1 1.4 4
42 3 1 0.58 1
43 1 1 1.24 1
43 2 1 1.09 1
43 3 1 1.08 4
43 4 1 1.37 2
44 1 0 1.37 3
44 2 0 2.32 3
44 3 0 1.51 3
44 4 0 2.1 2
45 1 1 0.17 4
45 2 1 0.36 4
45 3 1 0.89 3
45 4 1 0.39 4
46 1 1 1.4 2
46 2 1 1.35 2
46 3 1 1.18 2
46 4 1 0.39 4
47 1 1 0.24 2
47 2 1 0.41 4
47 3 1 -0.54 4
47 4 1 0.24 4
48 1 1 -0.06 3
48 2 1 -0.68 3
48 4 1 -0.45 4
49 1 0 0.68 1
49 2 0 0.2 1
49 3 0 1 3
49 4 0 0.39 4
50 1 1 -1.39 4
50 3 1 -1.19 2
50 4 1 -0.96 4
51 1 0 2.16 3
51 2 0 2.3 1
51 3 0 2 4
52 1 0 0.08 1
52 2 0 0.43 1
52 3 0 0.23 2
52 4 0 0.24 2
53 1 1 -0.46 2
53 3 1 -0.53 3
54 1 1 1.69 3
54 2 1 1.89 4
54 3 1 1.91 4
54 4 1 2.36 4
55 1 0 -1.23 1
55 2 0 -1.82 1
55 3 0 -1.26 3
55 4 0 -1.43 1
56 1 1 0.06 2
56 3 1 -0.14 1
56 4 1 0.06 3
57 1 0 -0.25 2
57 2 0 -0.58 2
57 3 0 -0.42 3
57 4 0 -0.48 2
58 1 1 -0.27 2
58 3 1 -0.7 2
59 1 1 -1.43 2
59 2 1 -0.73 2
59 4 1 -1.23 3
60 1 1 -0.55 3
60 2 1 -0.7 1
60 3 1 -1.22 4
60 4 1 -1.04 3
61 1 0 1.63 2
61 2 0 1.78 4
61 3 0 1.92 4
61 4 0 1.31 2
62 1 1 0.52 1
62 2 1 0.58 3
62 3 1 0.71 2
62 4 1 0.48 3
63 1 0 -0.22 1
63 2 0 -0.41 1
63 3 0 0.15 2
63 4 0 -0.37 2
64 1 1 1.27 3
64 2 1 1.12 3
64 3 1 1.54 3
64 4 1 1.84 3
65 1 0 0.73 2
65 2 0 0.19 2
65 3 0 0.56 2
65 4 0 1.06 4
66 1 1 0.15 1
66 2 1 0.37 4
66 3 1 -0.13 2
66 4 1 0.22 3
67 1 1 -3.2 2
67 2 1 -2.65 1
67 3 1 -2.84 3
68 1 0 0.1 3
68 2 0 -0.51 3
68 3 0 -0.13 3
68 4 0 0.29 4
69 1 1 -0.29 4
69 2 1 -0.57 4
69 3 1 -0.24 2
69 4 1 -0.7 4
70 1 0 -0.52 3
70 2 0 -0.68 2
70 3 0 -0.38 3
END DATA.
";

fn run(syntax: &str) -> Vec<Table> {
    let mut engine = Engine::default();
    let (commands, errors) = parse(&format!("{DATA}{syntax}"));
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

fn cell(tables: &[Table], title: &str, row: &str, column: &str) -> f64 {
    let table = tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"));
    let r = table
        .rows
        .iter()
        .position(|r| matches!(&r[0], Cell::Text(t) if t == row))
        .unwrap_or_else(|| panic!("{title}: no row {row}"));
    let c = table
        .column_titles()
        .iter()
        .position(|t| t == column)
        .unwrap_or_else(|| panic!("{title}: no column {column}"));
    match &table.rows[r][c] {
        Cell::Number(n) => *n,
        other => panic!("{title} / {row} / {column}: {other:?}"),
    }
}

fn near(actual: f64, expected: f64, tolerance: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= tolerance * (1.0 + expected.abs()),
        "{what}: {actual} is not {expected}"
    );
}

/// The estimates, the robust and the model-based standard errors of each parameter.
fn check(corrtype: &str, link: &str, extra: &str, expected: &[(&str, f64, f64, f64)]) {
    let source = |covb: &str| {
        format!(
            "COMPUTE t = wave.
GENLIN y (ORDER=ASCENDING) WITH arm wave stress /MODEL arm wave stress DISTRIBUTION=MULTINOMIAL LINK={link}
  /REPEATED SUBJECT=id WITHINSUBJECT=t CORRTYPE={corrtype} COVB={covb} MAXITERATIONS=500 PCONVERGE=1E-12(ABSOLUTE){extra}
  /PRINT SOLUTION."
        )
    };
    let robust = run(&source("ROBUST"));
    let model = run(&source("MODEL"));
    for (row, b, robust_se, model_se) in expected {
        near(
            cell(&robust, "Parameter Estimates", row, "B"),
            *b,
            1e-6,
            row,
        );
        near(
            cell(&robust, "Parameter Estimates", row, "Std. Error"),
            *robust_se,
            1e-6,
            row,
        );
        near(cell(&model, "Parameter Estimates", row, "B"), *b, 1e-6, row);
        near(
            cell(&model, "Parameter Estimates", row, "Std. Error"),
            *model_se,
            1e-6,
            row,
        );
    }
}

#[test]
fn exchangeable_association_with_the_logit_link_agrees_with_the_estimating_equations_in_r() {
    check(
        "EXCHANGEABLE",
        "CUMLOGIT",
        "",
        &[
            (
                "Threshold [y = 1]",
                -0.299078092781,
                0.35973706327,
                0.32527830414,
            ),
            (
                "Threshold [y = 2]",
                0.915317985283,
                0.366089996509,
                0.324697011452,
            ),
            (
                "Threshold [y = 3]",
                2.17092268344,
                0.410808744131,
                0.364300133331,
            ),
            ("arm", 0.519097796248, 0.327100594949, 0.290370010553),
            ("wave", 0.32681196013, 0.0853668470682, 0.0866245393454),
            ("stress", 0.59471531739, 0.127972765692, 0.138208095478),
        ],
    );
}

#[test]
fn ar1_association_with_the_logit_link_agrees_with_the_estimating_equations_in_r() {
    check(
        "AR(1)",
        "CUMLOGIT",
        "",
        &[
            (
                "Threshold [y = 1]",
                -0.251732797425,
                0.352493304938,
                0.336017562849,
            ),
            (
                "Threshold [y = 2]",
                0.959571898407,
                0.35787346857,
                0.336593711135,
            ),
            (
                "Threshold [y = 3]",
                2.19528392589,
                0.399492573199,
                0.370015433217,
            ),
            ("arm", 0.576698322202, 0.331126442163, 0.264023288892),
            ("wave", 0.347789829186, 0.0858941125092, 0.105312239314),
            ("stress", 0.537432384423, 0.130261979967, 0.127914377185),
        ],
    );
}

#[test]
fn two_dependent_association_with_the_logit_link_agrees_with_the_estimating_equations_in_r() {
    check(
        "MDEPENDENT(2)",
        "CUMLOGIT",
        "",
        &[
            (
                "Threshold [y = 1]",
                -0.275200655468,
                0.35437160968,
                0.333061835323,
            ),
            (
                "Threshold [y = 2]",
                0.947035041446,
                0.358920623379,
                0.335435281247,
            ),
            (
                "Threshold [y = 3]",
                2.15600232661,
                0.401191679682,
                0.368126249877,
            ),
            ("arm", 0.561135670142, 0.340358065517, 0.280499965518),
            ("wave", 0.354694361207, 0.0895904148763, 0.097641148173),
            ("stress", 0.527095524298, 0.142110855448, 0.133330321494),
        ],
    );
}

#[test]
fn unstructured_association_with_the_logit_link_agrees_with_the_estimating_equations_in_r() {
    check(
        "UNSTRUCTURED",
        "CUMLOGIT",
        "",
        &[
            (
                "Threshold [y = 1]",
                -0.374122675346,
                0.465202961128,
                0.290687993708,
            ),
            (
                "Threshold [y = 2]",
                0.92706246475,
                0.466388982073,
                0.304290544619,
            ),
            (
                "Threshold [y = 3]",
                2.38694978046,
                0.506492049546,
                0.381324795281,
            ),
            ("arm", 0.436761977147, 0.409819724852, 0.260154329227),
            ("wave", 0.299402290331, 0.11495655432, 0.0731707407532),
            ("stress", 0.683521537048, 0.16451329371, 0.127939577141),
        ],
    );
}

#[test]
fn exchangeable_association_with_the_probit_link_agrees_with_the_estimating_equations_in_r() {
    check(
        "EXCHANGEABLE",
        "CUMPROBIT",
        "",
        &[
            (
                "Threshold [y = 1]",
                -0.210338138937,
                0.216819948314,
                0.195537389815,
            ),
            (
                "Threshold [y = 2]",
                0.516126942399,
                0.220841726854,
                0.195770314237,
            ),
            (
                "Threshold [y = 3]",
                1.28449231347,
                0.244826402981,
                0.214327559631,
            ),
            ("arm", 0.299102687272, 0.194348031024, 0.176318549189),
            ("wave", 0.185302862501, 0.0521655142215, 0.050608402787),
            ("stress", 0.363635262342, 0.0754655039144, 0.082339894617),
        ],
    );
}

#[test]
fn ar1_association_with_the_complementary_log_log_link_and_no_adjustment_agrees_with_the_estimating_equations_in_r()
 {
    check(
        "AR(1)",
        "CUMCLOGLOG",
        " ADJUSTCORR=NO",
        &[
            (
                "Threshold [y = 1]",
                -0.724260089445,
                0.264939243137,
                0.238792822403,
            ),
            (
                "Threshold [y = 2]",
                0.228354946754,
                0.235549249734,
                0.216237135632,
            ),
            (
                "Threshold [y = 3]",
                1.03661736286,
                0.259806349627,
                0.224135244123,
            ),
            ("arm", 0.423049316901, 0.225677689721, 0.179687684191),
            ("wave", 0.21509807977, 0.0642026720197, 0.0699283895929),
            ("stress", 0.330109581182, 0.0920892809829, 0.0869369210139),
        ],
    );
}

// The independent structure is the plain ordinal model: R's `multgee::ordLORgee` with independent
// local odds ratios gives the same estimates (its coefficients have the opposite sign) and the same
// robust standard errors, and the script's model-based ones are the inverse of the information.
#[test]
fn the_independent_structure_agrees_with_multgee() {
    check(
        "INDEPENDENT",
        "CUMLOGIT",
        "",
        &[
            ("Threshold [y = 1]", -0.27008282, 0.35498540, 0.31353570),
            ("Threshold [y = 2]", 0.95669252, 0.36131534, 0.31544368),
            ("Threshold [y = 3]", 2.19723280, 0.39921395, 0.33858802),
            ("arm", 0.60059220, 0.32833454, 0.23369968),
            ("wave", 0.34087513, 0.08610969, 0.10255717),
            ("stress", 0.55991711, 0.13132534, 0.11557234),
        ],
    );
}

// PRINT WORKINGCORR shows the estimated association: the exchangeable matrix is the same block
// between every two measurements, and the blocks of one measurement with itself are empty.
#[test]
fn the_working_association_is_printed() {
    let tables = run(
        "COMPUTE t = wave.
GENLIN y (ORDER=ASCENDING) WITH arm wave stress /MODEL arm wave stress DISTRIBUTION=MULTINOMIAL
  /REPEATED SUBJECT=id WITHINSUBJECT=t CORRTYPE=EXCHANGEABLE PCONVERGE=1E-12(ABSOLUTE) MAXITERATIONS=500
  /PRINT WORKINGCORR.",
    );
    let table = tables
        .iter()
        .find(|t| t.title == "Working Association Between Measurements")
        .expect("the association table");
    // 4 positions of 3 categories
    assert_eq!(table.rows.len(), 12);
    let alpha = [
        [0.159461299802, 0.0197914844606, -0.0257238920246],
        [0.0197914844606, 0.0473108801868, 0.000995355882601],
        [-0.0257238920246, 0.000995355882601, 0.134172085899],
    ];
    let number = |r: usize, c: usize| match &table.rows[r][c] {
        Cell::Number(n) => *n,
        other => panic!("row {r} column {c}: {other:?}"),
    };
    for u in 0..4 {
        for v in 0..4 {
            for (j, expected) in alpha.iter().enumerate() {
                for (l, want) in expected.iter().enumerate() {
                    let cell = &table.rows[u * 3 + j][1 + v * 3 + l];
                    if u == v {
                        assert!(matches!(cell, Cell::Empty), "{u} {v} {j} {l}: {cell:?}");
                    } else {
                        near(number(u * 3 + j, 1 + v * 3 + l), *want, 1e-6, "alpha");
                    }
                }
            }
        }
    }
}

// A subject whose working matrix is not positive definite is moved toward the identity, and the
// warning counts the subjects (4 in this file at the exchangeable fit).
#[test]
fn subjects_with_a_matrix_that_is_not_positive_definite_are_counted() {
    let mut engine = Engine::default();
    let (commands, errors) = parse(&format!(
        "{DATA}COMPUTE t = wave.
GENLIN y (ORDER=ASCENDING) WITH arm wave stress /MODEL arm wave stress DISTRIBUTION=MULTINOMIAL
  /REPEATED SUBJECT=id WITHINSUBJECT=t CORRTYPE=EXCHANGEABLE PCONVERGE=1E-12(ABSOLUTE) MAXITERATIONS=500."
    ));
    assert!(errors.is_empty(), "{errors:?}");
    for command in &commands {
        engine.execute(&command.command).expect("runs");
    }
    let warnings = engine.warnings().join(" ");
    assert!(
        warnings.contains("of 4 subject(s) was not positive definite"),
        "{warnings}"
    );
}
