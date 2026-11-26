//! UNIANOVA POSTHOC against R in a model of two factors, unbalanced (34 cases: factor a with 11,
//! 14 and 9 cases, factor b with two levels, a covariate x). The comparisons of a post hoc test
//! use the observed means of the factor and the error mean square and df of the model, or the mean
//! square of another effect with VS. The expected numbers are those formulas written out in R
//! (`lm` for the model; `pt`, `ptukey`, `pf` and the studentized maximum modulus by `integrate`
//! for the tests): the first test with the full-factorial model's error (28 df), the others with
//! the Type III mean square of b as the error term, and with the covariate in the model. The
//! scripts are in lab/oracles/glm-posthoc-r.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /y a b x.
BEGIN DATA.
9.2 1 2 8
10.7 1 1 8.3
10.2 1 2 7.3
9 1 1 6.9
9.3 1 1 6
9.6 1 2 8.4
10.2 1 1 7.2
11.1 1 1 8.9
9.6 1 1 7.9
11.5 1 1 8.5
10.8 1 2 7.8
11.9 2 2 7.4
13.8 2 2 10.2
11.5 2 1 8.2
13.2 2 1 7.9
13.4 2 2 7.6
12.3 2 2 9.3
10.8 2 1 7.6
11 2 2 8.2
16.1 2 2 10.2
11.2 2 1 7.4
12 2 2 7.7
11.5 2 1 9.5
11.3 2 2 8.7
13.7 2 2 10.2
14.3 3 2 8.9
11.3 3 2 8.5
11.4 3 1 7.8
14.5 3 1 11.4
10 3 1 9.6
10.9 3 1 7.2
11.9 3 2 6.7
14.6 3 2 11.5
11.5 3 1 9.6
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

fn near(got: f64, want: f64, what: &str) {
    assert!(
        (got - want).abs() <= 1e-8 + 1e-7 * want.abs(),
        "{what}: {got} vs {want}"
    );
}

fn number(c: &Cell) -> f64 {
    match c {
        Cell::Number(n) => *n,
        other => panic!("{other:?}"),
    }
}

/// The rows of a test in "Multiple Comparisons: y" in the order (1,2), (1,3), (2,3): the
/// difference, standard error, significance and limits.
fn pairs(tables: &[Table], test: &str) -> Vec<[f64; 5]> {
    let t = tables
        .iter()
        .find(|t| t.title == "Multiple Comparisons: y")
        .expect("table");
    let at = t
        .rows
        .iter()
        .position(|r| matches!(&r[0], Cell::Text(s) if s == test))
        .unwrap_or_else(|| panic!("no {test}"));
    // each test lists 6 ordered pairs: 1-2, 1-3, 2-1, 2-3, 3-1, 3-2; the three with I < J
    [0, 1, 3]
        .iter()
        .map(|&k| {
            let r = &t.rows[at + k];
            [
                number(&r[3]),
                number(&r[4]),
                number(&r[5]),
                number(&r[6]),
                number(&r[7]),
            ]
        })
        .collect()
}

#[test]
fn the_comparisons_use_the_models_error_term() {
    // observed means 10.109091, 12.407143 and 12.266667 over 11, 14 and 9 cases; the error mean
    // square is 1.795768 on 28 df
    let t = run("UNIANOVA y BY a b /POSTHOC=a(LSD BONFERRONI SIDAK SCHEFFE TUKEY GT2 GABRIEL).");
    let expected: [(&str, [[f64; 5]; 3]); 6] = [
        (
            "LSD",
            [
                [
                    -2.298051948,
                    0.539926621,
                    0.000210655,
                    -3.404041494,
                    -1.192062402,
                ],
                [
                    -2.157575758,
                    0.602313361,
                    0.001272375,
                    -3.391358749,
                    -0.923792767,
                ],
                [
                    0.140476190,
                    0.572537243,
                    0.807969253,
                    -1.032313187,
                    1.313265568,
                ],
            ],
        ),
        (
            "Bonferroni",
            [
                [
                    -2.298051948,
                    0.539926621,
                    0.000631965,
                    -3.672956311,
                    -0.923147585,
                ],
                [
                    -2.157575758,
                    0.602313361,
                    0.003817126,
                    -3.691345786,
                    -0.623805730,
                ],
                [0.140476190, 0.572537243, 1.0, -1.317469987, 1.598422368],
            ],
        ),
        (
            "Sidak",
            [
                [
                    -2.298051948,
                    0.539926621,
                    0.000631832,
                    -3.668987989,
                    -0.927115907,
                ],
                [
                    -2.157575758,
                    0.602313361,
                    0.003812271,
                    -3.686918937,
                    -0.628232578,
                ],
                [
                    0.140476190,
                    0.572537243,
                    0.992918711,
                    -1.313261986,
                    1.594214367,
                ],
            ],
        ),
        (
            "Scheffe",
            [
                [
                    -2.298051948,
                    0.539926621,
                    0.000925457,
                    -3.693610419,
                    -0.902493477,
                ],
                [
                    -2.157575758,
                    0.602313361,
                    0.005084347,
                    -3.714386408,
                    -0.600765107,
                ],
                [
                    0.140476190,
                    0.572537243,
                    0.970379749,
                    -1.339371567,
                    1.620323948,
                ],
            ],
        ),
        (
            "Tukey HSD",
            [
                [
                    -2.298051948,
                    0.539926621,
                    0.000599694,
                    -3.634019524,
                    -0.962084372,
                ],
                [
                    -2.157575758,
                    0.602313361,
                    0.003531675,
                    -3.647909981,
                    -0.667241534,
                ],
                [
                    0.140476190,
                    0.572537243,
                    0.967396988,
                    -1.276181486,
                    1.557133867,
                ],
            ],
        ),
        // the maximum modulus tests: the significance only (their limits follow their own
        // critical values, which the ONEWAY audit checks)
        (
            "Hochberg",
            [
                [-2.298051948, 0.539926621, 0.000630108, f64::NAN, f64::NAN],
                [-2.157575758, 0.602313361, 0.003790900, f64::NAN, f64::NAN],
                [0.140476190, 0.572537243, 0.992549406, f64::NAN, f64::NAN],
            ],
        ),
    ];
    for (test, rows) in expected {
        let got = pairs(&t, test);
        for (g, want) in got.iter().zip(rows) {
            for (k, w) in want.iter().enumerate() {
                if !w.is_nan() {
                    near(g[k], *w, &format!("{test} column {k}"));
                }
            }
        }
    }
    let gabriel = pairs(&t, "Gabriel");
    for (g, p) in gabriel.iter().zip([0.000617108, 0.003746697, 0.992416765]) {
        near(g[2], p, "Gabriel");
    }
}

#[test]
fn the_homogeneous_subsets_use_the_harmonic_mean_of_the_groups() {
    // harmonic mean 10.970976; the second subset holds the means of groups 3 and 2
    let t = run("UNIANOVA y BY a b /POSTHOC=a(SNK DUNCAN TUKEY SCHEFFE GT2).");
    let subsets = t
        .iter()
        .find(|t| t.title == "Homogeneous Subsets: y")
        .expect("subsets");
    for (test, sig) in [
        ("Student-Newman-Keuls", 0.807845235),
        ("Duncan", 0.807845235),
        ("Tukey HSD", 0.967354741),
        ("Scheffe", 0.970341299),
        ("Hochberg", 0.992534981),
    ] {
        let at = subsets
            .rows
            .iter()
            .position(|r| matches!(&r[0], Cell::Text(s) if s == test))
            .unwrap_or_else(|| panic!("no {test}"));
        // three groups, then the Sig. row
        let row = &subsets.rows[at + 3];
        near(number(&row[4]), sig, test);
        assert_eq!(number(&row[3]), 1.0, "{test}");
    }
    assert!(subsets.caption.as_deref().unwrap().contains("10.971"));
}

#[test]
fn vs_takes_the_mean_square_of_another_effect_as_the_error() {
    // the Type III mean square of b is 4.617460 on 1 df
    let t = run("UNIANOVA y BY a b /POSTHOC=a(LSD BONFERRONI) VS b.");
    assert!(
        t.iter()
            .find(|t| t.title == "Multiple Comparisons: y")
            .unwrap()
            .caption
            .as_deref()
            .unwrap()
            .contains("mean square of b (1 df)")
    );
    for (test, ses, sigs) in [
        (
            "LSD",
            [0.865787350, 0.965826223, 0.918079389],
            [0.229375769, 0.267948866, 0.903339904],
        ),
        (
            "Bonferroni",
            [0.865787350, 0.965826223, 0.918079389],
            [0.688127307, 0.803846599, 1.0],
        ),
    ] {
        for (k, g) in pairs(&t, test).iter().enumerate() {
            near(g[1], ses[k], test);
            near(g[2], sigs[k], test);
        }
    }
}

#[test]
fn a_covariate_changes_the_error_term_but_not_the_means() {
    // error mean square 1.155127 on 27 df; the means stay the observed ones
    let t = run("UNIANOVA y BY a b WITH x /POSTHOC=a(LSD TUKEY).");
    for (test, sigs) in [
        ("LSD", [0.000013368, 0.000127701, 0.762011230]),
        ("Tukey HSD", [0.000038793, 0.000364885, 0.949814711]),
    ] {
        for (k, g) in pairs(&t, test).iter().enumerate() {
            near(g[1], [0.433036465, 0.483072400, 0.459191109][k], test);
            near(g[2], sigs[k], test);
        }
    }
}
